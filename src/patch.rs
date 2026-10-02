use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Context;
use file_utils::FileHandler;
use qbsdiff::Bspatch;

use crate::config::{AppConfig, ResourceRegistryConfig};
use crate::error::{AppError, Result};
use crate::manifest::ManiReader;
use crate::model::{Region, ResourceEntries};

pub struct PatchMerger {
    /// 区域列表
    regions: Vec<Region>,
    /// 待解包文件名
    unpack_filenames: Vec<String>,
    /// 清单输出目录
    manifest_output_dir: PathBuf,
    /// 补丁清单文件名
    patch_manifest_filename: String,
    /// 补丁来源目录
    patch_source_dir: PathBuf,
    /// 解包输出目录
    unpack_output_dir: PathBuf,
    /// 根清单文件名
    root_mani_filename: String,
    /// 附加清单文件名
    add_manifest_filename: String,
    /// 资源补丁标签
    update_resource_patch_tag: String,
    /// 清单补丁标签
    update_manifest_patch_tag: String,
    /// 清单扩展名
    manifest_extension: String,
    /// 是否启用正则过滤
    use_resource_regex: bool,
    /// 资源注册表
    registry: ResourceRegistryConfig,
    /// 补丁清单缓存
    manifest_cache: RefCell<HashMap<Region, Rc<ResourceEntries>>>,
    /// 清单文件缓存
    mani_cache: RefCell<HashMap<PathBuf, Rc<ManiReader>>>,
}

impl PatchMerger {
    pub fn from_app_config(app: &AppConfig) -> Self {
        Self {
            regions: app.regions(),
            unpack_filenames: app.resource_registry.unpack_files.clone(),
            manifest_output_dir: PathBuf::from(&app.file_path.manifest_output_dir),
            patch_manifest_filename: app.file_name.patch_manifest_file.clone(),
            patch_source_dir: PathBuf::from(&app.file_path.patch_output_dir),
            unpack_output_dir: PathBuf::from(&app.file_path.unpack_output_dir),
            root_mani_filename: app.file_name.update_root_mani_file.clone(),
            add_manifest_filename: app.file_name.update_add_manifest_name.clone(),
            update_resource_patch_tag: app.file_name.update_resource_patch_tag_suffix.clone(),
            update_manifest_patch_tag: app.file_name.update_manifest_patch_tag_suffix.clone(),
            manifest_extension: app.file_name.update_manifest_extension.clone(),
            use_resource_regex: app.feature_flags.is_use_resource_regex,
            registry: app.resource_registry.clone(),
            manifest_cache: RefCell::new(HashMap::new()),
            mani_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn apply_all(&self) -> Result<()> {
        for region in &self.regions {
            logger::head!("MERGE PATCHES {region}");
            self.apply_for_region(region)?;
        }

        Ok(())
    }

    pub fn apply_for_region(&self, region: &Region) -> Result<()> {
        for file_name in self.unpack_targets(region)? {
            let patches = self.patches_for(region, &file_name)?;

            if patches.is_empty() {
                self.copy_original_file(region, &file_name)?;
                continue;
            }

            logger::step!("[{region}] {file_name}");
            self.merge_patch(region, &file_name, &patches)
                .with_context(|| {
                    format!("Failed to apply patch for '{file_name}' in region '{region}'")
                })?;
        }

        Ok(())
    }

    fn unpack_targets(&self, region: &Region) -> Result<Vec<String>> {
        if !self.use_resource_regex {
            return Ok(self.unpack_filenames.clone());
        }

        let manifest = self.load_patch_manifest(region)?;

        Ok(manifest
            .keys()
            .filter(|key| self.registry.is_base_resource(key))
            .filter(|key| !key.ends_with(&self.manifest_extension))
            .cloned()
            .collect())
    }

    fn patches_for(&self, region: &Region, file_name: &str) -> Result<Vec<String>> {
        let manifest = self.load_patch_manifest(region)?;
        let suffix = format!(".{file_name}");

        let mut patches: Vec<String> = manifest
            .keys()
            .filter(|key| key.as_str() != file_name && key.ends_with(&suffix))
            .cloned()
            .collect();

        patches.sort_by_key(|name| Self::patch_sequence(name));

        Ok(patches)
    }

    fn merge_patch(&self, region: &Region, file_name: &str, patches: &[String]) -> Result<()> {
        let origin_path = self.patch_path(region, file_name);
        self.ensure_origin_exists(&origin_path)?;

        let output_path = self.unpacked_path(region, file_name);
        FileHandler::resolve_path(&output_path)?;

        let latest_patch = patches
            .iter()
            .max_by_key(|name| Self::patch_sequence(name))
            .ok_or_else(|| AppError::message("patch list is empty"))?;

        if self.verify_cached_output(region, file_name, latest_patch, &output_path)? {
            log::info!("[{region}] {file_name} cached output verified, skipping merge");
            return Ok(());
        }

        for (step, patch_name) in patches.iter().enumerate() {
            self.merge_patch_step(
                region,
                file_name,
                patch_name,
                step,
                &origin_path,
                &output_path,
            )
            .with_context(|| format!("Merge step {step} failed for patch: {patch_name}"))?;
        }

        Ok(())
    }

    fn merge_patch_step(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
        step: usize,
        origin_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        let patch_path = self.patch_path(region, patch_name);
        let expected_hash = self.expected_resource_hash(region, file_name, patch_name)?;

        if step != 0 {
            log::debug!("{}", "-".repeat(64));
        }

        let patch_hash = FileHandler::compute_md5(&patch_path).with_context(|| {
            format!("Failed to compute MD5 for patch: {}", patch_path.display())
        })?;
        log::debug!("[{region}] {patch_name} hex={patch_hash}");

        let base_path = if step == 0 { origin_path } else { output_path };
        let base_data = Self::read_file(base_path)?;
        let patch_data = Self::read_file(&patch_path)?;

        let merged = Self::apply_bsdiff(&base_data, &patch_data, patch_name)?;
        std::fs::write(output_path, &merged)
            .with_context(|| format!("Failed to write merged result: {}", output_path.display()))?;

        log::debug!("[{region}] {file_name} merged hex={expected_hash}");
        if !Self::verify_hash(region, output_path, &expected_hash, Some(patch_name)) {
            return Err(AppError::message(format!(
                "Hash mismatch after merge: {patch_name}"
            )));
        }

        Ok(())
    }

    fn copy_original_file(&self, region: &Region, file_name: &str) -> Result<()> {
        let output_path = self.unpacked_path(region, file_name);
        let expected_hash = self.expected_original_hash(region, file_name)?;

        if output_path.is_file() {
            log::debug!("[{region}] {file_name} already exists, verifying expected hash");
            if Self::verify_hash(region, &output_path, &expected_hash, Some(file_name)) {
                return Ok(());
            }
            log::debug!("[{region}] {file_name} hash mismatch, overwriting with the source file");
        }

        let origin_path = self.patch_path(region, file_name);
        self.ensure_origin_exists(&origin_path)?;

        let origin_hash = FileHandler::compute_md5(&origin_path)
            .with_context(|| format!("Failed to compute MD5 for: {}", origin_path.display()))?;
        log::debug!("[{region}] {file_name} no merge required, hex={origin_hash}");

        FileHandler::copy_file(&origin_path, &output_path)?;

        if !Self::verify_hash(region, &output_path, &expected_hash, Some(file_name)) {
            return Err(AppError::message(format!(
                "Hash mismatch original file: {file_name}"
            )));
        }

        Ok(())
    }

    fn verify_cached_output(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
        output_path: &Path,
    ) -> Result<bool> {
        let expected_hash = self.expected_resource_hash(region, file_name, patch_name)?;

        if !output_path.is_file() {
            log::debug!("[{region}] {file_name} output not found, will generate");
            return Ok(false);
        }

        log::debug!("[{region}] {file_name} verifying cached output");
        log::debug!("[{region}] latest patch: {patch_name}");
        log::debug!("[{region}] expected hex: {expected_hash}");

        Ok(Self::verify_hash(
            region,
            output_path,
            &expected_hash,
            Some(file_name),
        ))
    }

    fn expected_resource_hash(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
    ) -> Result<String> {
        let mani_path = self.update_mani_path(region, file_name, patch_name);

        match self.read_resource_hash(region, &mani_path, file_name) {
            Ok(hash) => Ok(hash),
            Err(err) => self.fallback_hash(region, file_name, err),
        }
    }

    fn expected_original_hash(&self, region: &Region, file_name: &str) -> Result<String> {
        let mani_path = self.add_mani_path(region);

        match self.read_resource_hash(region, &mani_path, file_name) {
            Ok(hash) => Ok(hash),
            Err(err) if !self.use_resource_regex => Err(err),
            Err(err) => self.fallback_hash(region, file_name, err),
        }
    }

    fn fallback_hash(&self, region: &Region, file_name: &str, cause: AppError) -> Result<String> {
        if !self.use_resource_regex {
            logger::fail!(
                "[{region}] hash not found for {file_name}, falling back to patch manifest"
            );
        }

        let manifest = self.load_patch_manifest(region)?;

        manifest
            .get(file_name)
            .map(|entry| {
                log::debug!(
                    "[{region}] {file_name} hash fallback from patch manifest: {}",
                    entry.hash
                );
                entry.hash.clone()
            })
            .ok_or_else(|| {
                AppError::message(format!(
                    "[{region}] hash fallback failed for '{file_name}': missing from patch manifest too (cause: {cause})"
                ))
            })
    }

    fn patch_manifest_path(&self, region: &Region) -> PathBuf {
        self.manifest_output_dir
            .join(region.as_str())
            .join(&self.patch_manifest_filename)
    }

    fn patch_path(&self, region: &Region, patch_name: &str) -> PathBuf {
        self.patch_source_dir.join(region.as_str()).join(patch_name)
    }

    fn unpacked_path(&self, region: &Region, patch_name: &str) -> PathBuf {
        self.unpack_output_dir
            .join(region.as_str())
            .join(patch_name)
    }

    fn mani_file_path(&self, region: &Region, prefix: &str) -> PathBuf {
        let filename = format!("{prefix}.{}", self.root_mani_filename);
        self.patch_source_dir.join(region.as_str()).join(filename)
    }

    fn update_mani_path(&self, region: &Region, file_name: &str, patch_name: &str) -> PathBuf {
        let suffix = format!(".{file_name}");
        let base_name = patch_name.strip_suffix(&suffix).unwrap_or(patch_name);
        let mani_prefix = base_name.replace(
            &self.update_resource_patch_tag,
            &self.update_manifest_patch_tag,
        );

        self.mani_file_path(region, &mani_prefix)
    }

    fn add_mani_path(&self, region: &Region) -> PathBuf {
        let filename = if self.use_resource_regex {
            &self.root_mani_filename
        } else {
            &self.add_manifest_filename
        };

        self.patch_source_dir.join(region.as_str()).join(filename)
    }

    fn load_patch_manifest(&self, region: &Region) -> Result<Rc<ResourceEntries>> {
        if let Some(cached) = self.manifest_cache.borrow().get(region) {
            return Ok(Rc::clone(cached));
        }

        let path = self.patch_manifest_path(region);
        let entries: ResourceEntries = FileHandler::read_json::<ResourceEntries>(&path)
            .with_context(|| format!("Failed to read patch manifest: {}", path.display()))?;

        let cached = Rc::new(entries);
        self.manifest_cache
            .borrow_mut()
            .insert(region.clone(), Rc::clone(&cached));

        Ok(cached)
    }

    fn load_mani(&self, region: &Region, mani_path: &Path) -> Result<Rc<ManiReader>> {
        if let Some(cached) = self.mani_cache.borrow().get(mani_path) {
            return Ok(Rc::clone(cached));
        }

        let reader = ManiReader::new(mani_path, Some(region))?;
        let cached = Rc::new(reader);
        self.mani_cache
            .borrow_mut()
            .insert(mani_path.to_path_buf(), Rc::clone(&cached));

        Ok(cached)
    }

    fn read_resource_hash(
        &self,
        region: &Region,
        mani_path: &Path,
        file_name: &str,
    ) -> Result<String> {
        let reader = self.load_mani(region, mani_path)?;

        reader
            .get_resource_hash_by_patch(file_name)
            .map(|hash| hash.to_string())
            .ok_or_else(|| {
                AppError::message(format!(
                    "Resource '{file_name}' not found in mani: {}",
                    mani_path.display()
                ))
            })
    }

    fn read_file(path: &Path) -> Result<Vec<u8>> {
        std::fs::read(path)
            .with_context(|| format!("Failed to read: {}", path.display()))
            .map_err(AppError::from)
    }

    fn ensure_origin_exists(&self, origin_path: &Path) -> Result<()> {
        if origin_path.is_file() {
            return Ok(());
        }

        Err(AppError::message(format!(
            "Origin patch not found: {}",
            origin_path.display()
        )))
    }

    fn apply_bsdiff(base: &[u8], patch: &[u8], patch_name: &str) -> Result<Vec<u8>> {
        let patcher =
            Bspatch::new(patch).with_context(|| format!("Invalid bsdiff patch: {patch_name}"))?;

        let mut output = Vec::with_capacity(patcher.hint_target_size() as usize);
        patcher
            .apply(base, Cursor::new(&mut output))
            .with_context(|| format!("bsdiff apply failed: {patch_name}"))?;

        Ok(output)
    }

    fn check_hash(path: &Path, expected: &str) -> Option<bool> {
        if !path.is_file() {
            return None;
        }
        FileHandler::compute_md5(path).map(|actual| actual.eq_ignore_ascii_case(expected))
    }

    fn verify_hash(
        region: &Region,
        path: impl AsRef<Path>,
        expected: &str,
        label: Option<&str>,
    ) -> bool {
        let path = path.as_ref();
        let label = label.unwrap_or_default();

        match Self::check_hash(path, expected) {
            Some(true) => {
                log::info!("[{region}] Hash OK: {label} {}", path.display());
                true
            }
            Some(false) => {
                let actual = FileHandler::compute_md5(path).unwrap_or_default();
                log::warn!(
                    "[{region}] Hash MISMATCH: {label} | actual={actual} expected={expected} | {}",
                    path.display()
                );
                false
            }
            None => {
                if !path.is_file() {
                    log::warn!(
                        "[{region}] File not found for hash verification: {}",
                        path.display()
                    );
                } else {
                    log::error!("[{region}] Failed to compute MD5: {}", path.display());
                }
                false
            }
        }
    }

    fn patch_sequence(name: &str) -> u64 {
        match name.split('_').nth(1).and_then(|seq| seq.parse().ok()) {
            Some(sequence) => sequence,
            None => {
                log::warn!("Invalid patch filename format: {name}");
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PatchMerger;

    #[test]
    fn parses_patch_sequence() {
        assert_eq!(PatchMerger::patch_sequence("p_12_u.lua.arcx"), 12);
        assert_eq!(PatchMerger::patch_sequence("p_1_m.ss_win.mani"), 1);
        assert_eq!(PatchMerger::patch_sequence("no-seq-here"), 0);
    }

    #[test]
    fn patch_sequence_sorts_numerically() {
        let mut names = vec![
            "p_2_u.lua.arcx".to_string(),
            "p_10_u.lua.arcx".to_string(),
            "p_1_u.lua.arcx".to_string(),
        ];
        names.sort_by_key(|name| PatchMerger::patch_sequence(name));

        assert_eq!(
            names,
            vec!["p_1_u.lua.arcx", "p_2_u.lua.arcx", "p_10_u.lua.arcx"]
        );
    }
}
