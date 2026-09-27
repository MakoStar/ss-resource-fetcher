use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashSet;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use qbsdiff::Bspatch;
use file_utils::FileHandler;

use crate::config::AppConfig;
use crate::config::FilePathConfigs;
use crate::config::FileNameConfigs;
use crate::extractor::TResEntryIndexMap;
use crate::utils::ManiFileReader;
use crate::config::ResourceRegistryConfig;

/// 根据补丁清单将热修复补丁合并到目标目录
pub struct HotfixPatchMerger {
    unpack_filenames: HashSet<&'static str>,
    manifest_output_dir: String,
    patch_manifest_filename: String,
    patch_source_dir: String,
    unpack_output_dir: String,
    root_mani_filename: String,
    add_manifest_filename: String,
    update_resource_patch_tag: String,
    update_manifest_patch_tag: String,
    app_config: &'static AppConfig,
}

impl HotfixPatchMerger {
    pub fn new() -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let filename_cfg: FileNameConfigs = app_config.file_name.clone();
        let filepath_cfg: FilePathConfigs = app_config.file_path.clone();

        Self {
            unpack_filenames: app_config.resource_registry.unpack_set().clone(),
            manifest_output_dir: filepath_cfg.manifest_output_dir,
            patch_manifest_filename: filename_cfg.patch_manifest_file,
            patch_source_dir: filepath_cfg.patch_output_dir,
            unpack_output_dir: filepath_cfg.unpack_output_dir,
            root_mani_filename: filename_cfg.update_root_mani_file,
            add_manifest_filename: filename_cfg.update_add_manifest_name,
            update_resource_patch_tag: filename_cfg.update_resource_patch_tag_suffix,
            update_manifest_patch_tag: filename_cfg.update_manifest_patch_tag_suffix,
            app_config,
        }
    }

    /// 返回指定区域的补丁清单文件路径
    fn patch_manifest_path(&self, region: &str) -> PathBuf {
        Path::new(&self.manifest_output_dir)
            .join(region)
            .join(&self.patch_manifest_filename)
    }

    /// 读取指定区域的补丁清单,失败时 panic
    fn patch_manifest(&self, region: &str) -> TResEntryIndexMap {
        let path: PathBuf = self.patch_manifest_path(region);
        FileHandler::read_json::<TResEntryIndexMap>(&path)
            .unwrap_or_else(|| panic!("Failed to read patch manifest: {:?}", path))
    }

    /// 返回需合并到对应资源的增量补丁表 按序号排序
    fn patches_to_merge(&self, region: &str, file_name: &str) -> Vec<String> {
        let manifest: TResEntryIndexMap = self.patch_manifest(region);
        let suffix: String = format!(".{}", file_name);

        let mut patches: Vec<String> = manifest
            .keys()
            .filter(|k| *k != file_name && k.ends_with(&suffix))
            .cloned()
            .collect();

        patches.sort_by_key(|s| {
            s.split('_')
                .nth(1)
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    log::warn!("Invalid patch filename format: {s}");
                    0
                })
        });
        patches
    }

    /// 构建指定区域下补丁文件的完整路径
    fn patch_path(&self, region: &str, patch_name: &str) -> PathBuf {
        Path::new(&self.patch_source_dir)
            .join(region)
            .join(patch_name)
    }

    /// 构建指定区域下解包补丁文件的完整路径
    fn unpacked_patch_path(&self, region: &str, patch_name: &str) -> PathBuf {
        Path::new(&self.unpack_output_dir)
            .join(region)
            .join(patch_name)
    }

    /// 构建指定区域下 .mani 清单补丁文件的完整路径
    fn mani_file_path(&self, region: &str, prefix: &str) -> PathBuf {
        let filename: String = format!("{}.{}", prefix, self.root_mani_filename);
        Path::new(&self.patch_source_dir)
            .join(region)
            .join(filename)
    }

    /// 根据更新补丁名推导对应的 .mani 文件路径：去掉后缀后将 `_u` 替换为 `_m`
    fn update_mani_path(&self, region: &str, file_name: &str, patch_name: &str) -> PathBuf {
        let suffix: String = format!(".{}", file_name);
        let base_name: &str = patch_name.strip_suffix(&suffix).unwrap_or(patch_name);
        let mani_prefix: String = base_name.replace(
            &self.update_resource_patch_tag, 
            &self.update_manifest_patch_tag,
        );
        self.mani_file_path(region, &mani_prefix)
    }

    /// 不需要合并时用 ss_win_add.mani 清单读取 hash
    fn add_mani_path(&self, region: &str) -> PathBuf {
        if self.app_config.feature_flags.is_use_resource_regex {
           return Path::new(&self.patch_source_dir)
                .join(region)
                .join(&self.root_mani_filename);
        }
        Path::new(&self.patch_source_dir)
            .join(region)
            .join(&self.add_manifest_filename)
    }

    /// 纯哈希校验匹配返回Some(true) 不匹配返回Some(false) 异常返回None
    fn check_hash(&self, path: impl AsRef<Path>, expected: &str) -> Option<bool> {
        let file_path: &Path = path.as_ref();
        if !file_path.is_file() {
            return None;
        }
        let actual: String = FileHandler::compute_md5(file_path)?;
        Some(actual.eq_ignore_ascii_case(expected))
    }

    /// 校验文件哈希是否与预期一致 返回 true/false, 计算失败或文件不存在时返回 false 并记录日志
    fn verify_hash(&self, region: &str, path: impl AsRef<Path>, expected: &str, label: Option<&str>) -> bool {
        let file_path: &Path = path.as_ref();
        let label_suffix: String = label.map(|l| format!("{l} - ")).unwrap_or_default();
        match self.check_hash(&file_path, expected) {
            Some(true) => {
                log::info!("[{}] Hash OJBK: {label_suffix} {}", region, file_path.display());
                true
            }
            Some(false) => {
                let actual: String = FileHandler::compute_md5(file_path).unwrap_or_default();
                let label_str: String = label_suffix.trim_end_matches(" - ").to_string();
                log::warn!(
                    "[{}] Hash MISMATCH: {label_str} | actual={actual} expected={expected} | {}",
                    region,
                    file_path.display()
                );
                false
            }
            None => {
                if !file_path.is_file() {
                    log::warn!(
                        "[{}] File not found for hash verification: {}",
                        region,
                        file_path.display()
                    );
                } else {
                    log::error!(
                        "[{}] Failed to compute MD5: {}",
                        region,
                        file_path.display()
                    );
                }
                false
            }
        }
    }

    /// 检查是否已存在解包文件并比对预期哈希是否正确
    fn verify_cached_output(&self, region: &str, file_name: &str, patch_name: &str, output_path: &Path) -> Result<bool> {
        let expected_hash: String = self.expected_resource_hash(region, file_name, patch_name)?;

        if !output_path.is_file() {
            log::debug!("[{region}] {file_name} output not found, will generate");
            return Ok(false);
        }

        log::debug!("[{region}] {file_name} output exists, verifying hash");
        log::debug!("[{region}] latest patch: {patch_name}"); 
        log::debug!("[{region}] expected hex: {expected_hash}");

        if self.verify_hash(region, output_path, &expected_hash, Some(file_name)) {
            Ok(true)
        } else {
            log::debug!("[{region}] {file_name} hash mismatch, will regenerate");
            Ok(false)
        }
    }

    /// 按顺序对目标文件应用补丁链,每步以上一步输出为基线,最终结果写入解包文件保存路径
    fn merge_patch(&self, region: &str, file_name: &str, patch_merge_list: &[String]) -> Result<()> {
        let origin_path: PathBuf = self.patch_path(region, file_name);
        anyhow::ensure!(origin_path.is_file(), "Origin patch not found: {}", origin_path.display());
        
        let output_path: PathBuf = self.unpacked_patch_path(region, file_name);
        FileHandler::resolve_path(&output_path)?;

        let max_patch_name: &String = patch_merge_list
            .iter()
            .max_by_key(|name| {
                name.split('_')
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
            })
            .ok_or_else(|| anyhow::anyhow!("patch_merge_list is empty"))?;
        
        if self.verify_cached_output(region, file_name, max_patch_name, &output_path)? {
            log::info!("[{region}] {file_name} cached output is verifyed, skipping");
            return Ok(());
        }

        for (step, patch_name) in patch_merge_list.iter().enumerate() {
            self.merge_patch_step(region, file_name, patch_name, step, &origin_path, &output_path)
                .with_context(|| format!("Merge step {step} failed for patch: {patch_name}"))?;
        }

        Ok(())
    }

    /// 应用 bsdiff 合并补丁并返回合并后的数据
    fn apply_bsdiff(base: &[u8], patch: &[u8], patch_name: &str) -> Result<Vec<u8>> {
        let patcher: Bspatch<'_> = Bspatch::new(patch)
            .with_context(|| format!("Invalid bsdiff patch: {patch_name}"))?;
        let mut output: Vec<u8> = Vec::with_capacity(patcher.hint_target_size() as usize);
        patcher
            .apply(base, Cursor::new(&mut output))
            .with_context(|| format!("bsdiff apply failed: {patch_name}"))?;

        Ok(output)
    }

    /// 从指定清单文件中读取资源的预期哈希值
    fn read_resource_hash(&self, mani_path: &PathBuf, file_name: &str) -> Result<String> {
        let reader: ManiFileReader = ManiFileReader::new(mani_path)
            .with_context(|| format!("Failed to open mani file: {}", mani_path.display()))?;
        reader
            .get_resource_hash_by_patch(file_name)
            .ok_or_else(|| anyhow!("Resource '{file_name}' not found in mani: {}", mani_path.display()))
            .with_context(|| format!("Failed to read target hash from: {}", mani_path.display()))
            .map(|s| s.to_string())
    }

    /// 从更新(p_x_m.ss_win.mani)清单文件中获取资源合并后的预期哈希值
    fn expected_resource_hash(&self, region: &str, file_name: &str, patch_name: &str) -> Result<String> {
        let mani_path: PathBuf = self.update_mani_path(region, file_name, patch_name);
        match self.read_resource_hash(&mani_path, file_name) {
            Ok(hash) => Ok(hash),
            Err(err) => {
                if !self.app_config.feature_flags.is_use_resource_regex {
                    // return Err(err);
                    logger::fail!("[{region}] resource regex enabled but hash not found: {file_name}");
                }
                self.hash_from_patch_manifest(region, file_name)
                    .ok_or_else(|| {
                        err.context(format!(
                            "fallback to patch manifest failed: '{file_name}' not found either"
                        ))
                    })
            }
        }
    }

    /// 从补丁清单里取指定资源的 hash
    fn hash_from_patch_manifest(&self, region: &str, file_name: &str) -> Option<String> {
        let manifest: TResEntryIndexMap = self.patch_manifest(region);
        manifest.get(file_name).map(|entry| {
            log::debug!(
                "[{region}] {file_name} hash fallback from patch manifest: {}",
                entry.hash
            );
            entry.hash.clone()
        })
    }

    /// 从新增(ss_win_add_mani)清单文件中获取不需要资源合并的文件哈希值
    fn expected_resource_original_hash(&self, region: &str, file_name: &str) -> Result<String> {
        let mani_path: PathBuf = self.add_mani_path(region);
        match self.read_resource_hash(&mani_path, file_name) {
            Ok(hash) => Ok(hash),
            Err(err) => {
                if !self.app_config.feature_flags.is_use_resource_regex {
                    return Err(err);
                }
                self.hash_from_patch_manifest(region, file_name)
                    .ok_or_else(|| {
                        err.context(format!(
                            "fallback to patch manifest failed: '{file_name}' not found either"
                        ))
                    })
            }
        }
    }

    /// 读取整个文件到新缓冲区
    fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        std::fs::read(path).with_context(|| format!("Failed to read: {}", path.display()))
    }

    /// 执行单步 bsdiff 合并：读取基线与补丁, 应用差分, 写入结果并校验清单哈希
    fn merge_patch_step(&self, region: &str, file_name: &str, patch_name: &str, step: usize, origin_path: &Path, output_path: &Path) -> Result<()> {
        let patch_path: PathBuf = self.patch_path(region, patch_name);
        let expected_hash: String = self.expected_resource_hash(region, file_name, patch_name)?;

        if step != 0 { log::debug!("{}", "-".repeat(64));}

        log::debug!("[{region}] {patch_name} hex={}", FileHandler::compute_md5(&patch_path)
            .with_context(|| format!("Failed to compute MD5 for patch: {}", patch_path.display()))?);

        let base_path: &Path = if step == 0 { origin_path } else { output_path };
        let base_data: Vec<u8> = self.read_file(base_path)
            .with_context(|| format!("Failed to read base: {}", base_path.display()))?;
        let patch_data: Vec<u8> = self.read_file(&patch_path)
            .with_context(|| format!("Failed to read patch: {}", patch_path.display()))?;

        let merged_data: Vec<u8> = Self::apply_bsdiff(&base_data, &patch_data, patch_name)?;
        std::fs::write(output_path, &merged_data)
            .with_context(|| format!("Failed to write merged result: {}", output_path.display()))?;

        log::debug!("[{region}] {file_name} merged hex={expected_hash}");
        if !self.verify_hash(region, output_path, expected_hash.as_str(), Some(patch_name)) {
            anyhow::bail!("Hash mismatch after merge: {patch_name}");
        }

        Ok(())
    }

    fn copy_original_file(&self, region: &str, file_name: &str) -> Result<()> {
        let output_path: PathBuf = self.unpacked_patch_path(region, file_name);
        let expected_hash: String = self.expected_resource_original_hash(region, file_name)?;

        if output_path.exists() && output_path.is_file() {
            log::debug!("[{region}] {file_name} already exists, verifying expected hash");
            if self.verify_hash(region, &output_path, expected_hash.as_str(), Some(file_name)) {
                return Ok(());
            } else {
                log::debug!("[{region}] {file_name} target hash mismatch, overwriting source file with new file");
            }
        }

        let origin_path: PathBuf = self.patch_path(region, file_name);
        anyhow::ensure!(origin_path.is_file(), "Origin patch not found: {}", origin_path.display());

        log::debug!("[{region}] {file_name}  no merge required");
        log::debug!("[{region}] {file_name}  hex={}", FileHandler::compute_md5(&origin_path)
            .with_context(|| format!("Failed to compute MD5 for patch: {}", origin_path.display()))?);

        FileHandler::copy_file(origin_path, &output_path)?;
        
        if !self.verify_hash(region, output_path, expected_hash.as_str(), Some(file_name)) {
            anyhow::bail!("Hash mismatch original file: {file_name}");
        }
        Ok(())
    }

    /// 解析当前 region 下要解包合并的目标文件
    fn resolve_unpack_targets(&self, region: &str) -> Vec<String> {
        if !self.app_config.feature_flags.is_use_resource_regex {
            return self.unpack_filenames.iter().map(|s| (*s).to_string()).collect();
        }

        let registry: &ResourceRegistryConfig = &self.app_config.resource_registry;
        let manifest: TResEntryIndexMap = self.patch_manifest(region);
        let ext_mani:&String  = &self.app_config.file_name.update_manifest_extension;
        
        manifest
            .keys()
            .filter(|k| registry.is_base_resource(k))
            .filter(|k| !k.ends_with(ext_mani))
            .cloned()
            .collect()
    }

    /// 对指定区域的所有待解包文件依次应用补丁,跳过无补丁的文件
    pub fn apply_patches_for_region(&self, region: &str) -> Result<()> {
        let targets: Vec<String> = self.resolve_unpack_targets(region);
        for file_name in &targets {
            let patches: Vec<String> = self.patches_to_merge(region, file_name);
            if patches.is_empty() {
                self.copy_original_file(region, file_name)?;
                continue;
            }
            logger::step!("[{region}] {file_name}");
            self.merge_patch(region, file_name, &patches).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to apply patch for '{file_name}' in region '{region}': {e:#}"
                )
            })?;
        }

        Ok(())
    }

    /// 对所有可用区域依次应用补丁,遇到首个错误即停止
    pub fn apply_all_patches(&self) -> Result<()> {
        for region in self.app_config.available_regions() {
            logger::head!("MERGE PATCHES {}", region);
            self.apply_patches_for_region(region)?;
        }

        Ok(())
    }
}

impl Default for HotfixPatchMerger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_func() -> anyhow::Result<()> {
        logger::init_logger!();

        HotfixPatchMerger::new().apply_all_patches()?;

        let patch_dir: String = AppConfig::get().file_path.unpack_output_dir.clone();
        let patch_path: PathBuf = Path::new(&patch_dir).join("CN").join("lua.arcx");

        let Some(hex) = FileHandler::compute_md5(&patch_path) else {
            anyhow::bail!("Failed to compute md5 for: {}", patch_path.display());
        };
        logger::test!("merged={}", &hex);

        Ok(())
    }
}
