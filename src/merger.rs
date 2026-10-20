use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Context;
use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;
use qbsdiff::Bspatch;

use crate::config::{AppConfig, ClientRegistryConfig};
use crate::error::{AppError, Result};
use crate::manifest::ManiReader;
use crate::model::{PatchRef, Region, ResourceEntries, apply_order, tier_label};

pub struct PatchMerger {
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
    /// 平台子目录
    platform: String,
    /// 根清单文件名
    root_mani_filename: String,
    /// 附加清单文件名
    add_manifest_filename: String,
    /// 清单补丁标签
    update_manifest_patch_tag: String,
    /// 清单扩展名
    manifest_extension: String,
    /// 是否启用正则过滤
    use_resource_regex: bool,
    /// 资源注册表
    registry: ClientRegistryConfig,
    /// 补丁清单缓存
    manifest_cache: RefCell<HashMap<Region, Rc<ResourceEntries>>>,
    /// 清单文件缓存
    mani_cache: RefCell<HashMap<PathBuf, Rc<ManiReader>>>,
    /// 区域合并计划缓存
    plans_cache: RefCell<HashMap<Region, Rc<RegionPlans>>>,
    /// 本运行各资源已应用到的补丁步
    progress: RefCell<HashMap<(Region, String), usize>>,
    /// 解码后的完整清单文件名 用来查补丁条目的版本与附加路径
    decoded_manifest_filename: String,
    /// 各区域服务器地址
    servers: IndexMap<Region, String>,
    /// 资源接口路径
    resource_route: String,
    /// 请求客户端
    requester: HttpFetcher,
    /// 解码清单缓存
    decoded_cache: RefCell<HashMap<Region, Rc<ResourceEntries>>>,
    /// 按版本取回的远程清单缓存
    remote_cache: RefCell<HashMap<(Region, u64, String), Rc<ManiReader>>>,
}

struct RegionPlans {
    targets: Vec<String>,
    plans: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MergeStats {
    /// 完成合并的文件数
    pub merged: usize,
    /// 复制本体的文件数
    pub copied: usize,
    /// 命中缓存跳过的文件数
    pub cached: usize,
}

impl MergeStats {
    pub fn total(&self) -> usize {
        self.merged + self.copied + self.cached
    }

    pub fn add(&mut self, other: &Self) {
        self.merged += other.merged;
        self.copied += other.copied;
        self.cached += other.cached;
    }
}

impl PatchMerger {
    pub fn from_app_config(app: &AppConfig) -> Self {
        Self {
            unpack_filenames: app.task.client.registry.unpack_files.clone(),
            manifest_output_dir: PathBuf::from(&app.task.client.filepath.manifest),
            patch_manifest_filename: app.task.client.filename.patch_manifest.clone(),
            patch_source_dir: PathBuf::from(&app.task.client.filepath.patch),
            unpack_output_dir: PathBuf::from(&app.task.client.filepath.unpack),
            platform: app.global.default_platform.clone(),
            root_mani_filename: app.task.client.filename.root_mani.clone(),
            add_manifest_filename: app.task.client.filename.add_mani.clone(),
            update_manifest_patch_tag: app.updatetag.m_suffix.clone(),
            manifest_extension: app.extension.mani.clone(),
            use_resource_regex: app.task.client.flags.op.use_resource_regex,
            registry: app.task.client.registry.clone(),
            manifest_cache: RefCell::new(HashMap::new()),
            mani_cache: RefCell::new(HashMap::new()),
            plans_cache: RefCell::new(HashMap::new()),
            progress: RefCell::new(HashMap::new()),
            decoded_manifest_filename: app.task.client.filename.decoded_manifest.clone(),
            servers: app.server_urls(),
            resource_route: app.task.client.route.resource.clone(),
            requester: HttpFetcher::new(&app.build_fetcher_config()),
            decoded_cache: RefCell::new(HashMap::new()),
            remote_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn report_tiers(&self, region: &Region) -> Result<()> {
        let path = self.root_mani_path(region);
        if !path.exists() {
            log::warn!(
                "[{region}] root mani missing, skip tier report: {}",
                self.root_mani_filename
            );
            return Ok(());
        }

        let mani = self.load_mani(region, &path)?;
        let infos = mani.patch_infos();

        if infos.is_empty() {
            return Ok(());
        }

        logger::head!("Patch tiers {region}");

        let declared = mani.bin_diff_patch_version().unwrap_or(0);
        let files: u64 = infos.iter().map(|info| info.file_count).sum();
        let total: u64 = infos.iter().map(|info| info.patch_size).sum();

        log::info!(
            "[{region}] {} tier(s), {files} file(s), {:.2}MB",
            infos.len(),
            total as f64 / 1024.0 / 1024.0
        );

        for info in infos {
            log::debug!(
                "[{region}] tier {}/{} res={} files={} size={:.2}MB",
                info.patch_version,
                declared,
                info.resource_version,
                info.file_count,
                info.patch_size as f64 / 1024.0 / 1024.0
            );
        }

        if declared != infos.len() as u64 {
            log::warn!("[{region}] patch ver {declared} != {} tier(s)", infos.len());
        }

        if let Some(latest) = self.latest_tier(region) {
            if latest > declared {
                log::warn!("[{region}] tier {latest} beyond declared {declared}");
            }
        }

        if mani.clear_expired_files() {
            logger::tips!("[{region}] filter outdated patches, keep p{declared}, {files} file(s)");
        }

        Ok(())
    }

    fn latest_tier(&self, region: &Region) -> Option<u64> {
        let entries = self.load_patch_manifest(region).ok()?;

        entries
            .keys()
            .map(|name| self.registry.patch_sequence(name))
            .max()
    }

    fn root_mani_path(&self, region: &Region) -> PathBuf {
        self.patch_source_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(&self.root_mani_filename)
    }

    pub fn apply_tier(&self, region: &Region, tier: u64) -> Result<MergeStats> {
        let plans = self.load_plans(region)?;

        enum Job<'a> {
            Copy(&'a str),
            Steps {
                file: &'a str,
                plan: &'a [String],
                first: usize,
                last: usize,
            },
        }

        let mut jobs = Vec::new();

        for file_name in &plans.targets {
            let plan = &plans.plans[file_name];

            if tier == 0 {
                if plan.is_empty() {
                    jobs.push(Job::Copy(file_name));
                }
                continue;
            }

            let in_tier = |name: &str| self.registry.patch_sequence(name) == tier;
            let Some(first) = plan.iter().position(|name| in_tier(name)) else {
                continue;
            };
            let last = plan.iter().rposition(|name| in_tier(name)).unwrap_or(first);

            if self.applied_step(region, file_name) == Some(plan.len() - 1) {
                continue;
            }

            jobs.push(Job::Steps {
                file: file_name,
                plan,
                first,
                last,
            });
        }

        if jobs.is_empty() {
            return Ok(MergeStats::default());
        }

        logger::tier!("merge {} - {} job(s)", tier_label(tier), jobs.len());
        let mut stats = MergeStats::default();

        for job in jobs {
            match job {
                Job::Copy(file) => {
                    if self.copy_original_file(region, file)? {
                        logger::step!("[{region}] copy: {file}");
                        stats.copied += 1;
                    } else {
                        logger::step!("[{region}] cached: {file}");
                        stats.cached += 1;
                    }
                }
                Job::Steps {
                    file,
                    plan,
                    first,
                    last,
                } => {
                    let final_step = plan.len() - 1;
                    let applied = self.applied_step(region, file);

                    if applied == Some(final_step) {
                        logger::step!("[{region}] cached: {file}");
                        stats.cached += 1;
                        continue;
                    }

                    let output_path = self.unpacked_path(region, file);
                    if applied.is_none()
                        && self.verify_cached_output(
                            region,
                            file,
                            &plan[final_step],
                            &output_path,
                        )?
                    {
                        self.set_applied_step(region, file, final_step);
                        logger::step!("[{region}] cached: {file}");
                        stats.cached += 1;
                        continue;
                    }

                    logger::step!("[{region}] merge: {file}");
                    self.run_steps(region, file, plan, first, last)?;
                    stats.merged += 1;
                }
            }
        }

        Ok(stats)
    }

    fn run_steps(
        &self,
        region: &Region,
        file_name: &str,
        plan: &[String],
        first: usize,
        last: usize,
    ) -> Result<()> {
        let output_path = self.unpacked_path(region, file_name);
        FileHandler::resolve_path(&output_path)?;

        let applied = self.applied_step(region, file_name);
        let from_output = if let Some(step) = applied {
            step + 1 == first
        } else {
            self.output_matches_step(region, file_name, plan, first)
        };

        let origin_path = self.patch_path(region, file_name);
        let start = if from_output { first } else { 0 };
        let mut merged = from_output;

        for (offset, patch_name) in plan[start..=last].iter().enumerate() {
            let step = start + offset;

            if self.registry.is_new_file_patch(patch_name) {
                log::debug!("[{region}] {file_name} new file: {patch_name}, no merge");
                self.replace_with_new_file(region, file_name, patch_name, &output_path)?;
                merged = true;
                continue;
            }

            if !merged {
                self.ensure_origin_exists(&origin_path)?;
            }

            let base_path = if merged {
                output_path.as_path()
            } else {
                origin_path.as_path()
            };
            self.merge_patch_step(region, file_name, patch_name, base_path, &output_path)
                .with_context(|| format!("Merge step {step} failed for patch: {patch_name}"))?;
            merged = true;
        }

        self.set_applied_step(region, file_name, last);

        Ok(())
    }

    fn output_matches_step(
        &self,
        region: &Region,
        file_name: &str,
        plan: &[String],
        first: usize,
    ) -> bool {
        if first == 0 {
            return false;
        }

        let Ok(Some(expected)) = self.expected_resource_hash(region, file_name, &plan[first - 1])
        else {
            return false;
        };

        Self::check_hash(&self.unpacked_path(region, file_name), &expected) == Some(true)
    }

    fn applied_step(&self, region: &Region, file_name: &str) -> Option<usize> {
        self.progress
            .borrow()
            .get(&(region.clone(), file_name.to_string()))
            .copied()
    }

    fn set_applied_step(&self, region: &Region, file_name: &str, step: usize) {
        self.progress
            .borrow_mut()
            .insert((region.clone(), file_name.to_string()), step);
    }

    fn unpack_targets(&self, region: &Region) -> Result<Vec<String>> {
        if !self.use_resource_regex {
            return Ok(self.unpack_filenames.clone());
        }

        let manifest = self.load_patch_manifest(region)?;
        let names = manifest.keys().map(String::as_str);

        let mut targets: Vec<_> = names
            .clone()
            .filter(|key| self.registry.is_base_resource(key))
            .chain(
                names
                    .filter(|key| self.registry.is_new_file_patch(key))
                    .filter_map(|key| self.registry.patch_base_name(key)),
            )
            .filter(|name| !name.ends_with(&self.manifest_extension))
            .map(str::to_string)
            .collect();

        targets.sort();
        targets.dedup();

        Ok(targets)
    }

    fn load_plans(&self, region: &Region) -> Result<Rc<RegionPlans>> {
        if let Some(cached) = self.plans_cache.borrow().get(region) {
            return Ok(Rc::clone(cached));
        }

        let targets = self.unpack_targets(region)?;
        let mut plans = HashMap::new();

        for file_name in &targets {
            plans.insert(file_name.clone(), self.merge_plan(region, file_name)?);
        }

        let plans = Rc::new(RegionPlans { targets, plans });
        self.plans_cache
            .borrow_mut()
            .insert(region.clone(), Rc::clone(&plans));

        Ok(plans)
    }

    fn merge_plan(&self, region: &Region, file_name: &str) -> Result<Vec<String>> {
        let manifest = self.load_patch_manifest(region)?;
        let suffix = format!(".{file_name}");

        let patches: Vec<_> = manifest
            .iter()
            .filter(|(key, _)| key.as_str() != file_name && key.ends_with(&suffix))
            .map(|(key, entry)| PatchRef::new(&self.registry, key, entry.version))
            .collect();

        let base_version = manifest.get(file_name).map(|entry| entry.version);
        let plan = apply_order(base_version, &patches, |name| {
            self.registry.is_new_file_patch(name)
        });

        Ok(plan.into_iter().map(str::to_string).collect())
    }

    fn replace_with_new_file(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
        output_path: &Path,
    ) -> Result<()> {
        let patch_path = self.patch_path(region, patch_name);
        let expected_hash = self.expected_resource_hash(region, file_name, patch_name)?;

        FileHandler::copy_file(&patch_path, output_path)?;

        log::debug!("[{region}] {file_name} replaced by {patch_name} (new file)");
        if let Some(expected) = &expected_hash
            && !Self::verify_hash(region, output_path, expected, Some(patch_name))
        {
            return Err(AppError::message(format!(
                "Hash mismatch after replace: {patch_name}"
            )));
        }

        Ok(())
    }

    fn merge_patch_step(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
        base_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        let patch_path = self.patch_path(region, patch_name);
        let expected_hash = self.expected_resource_hash(region, file_name, patch_name)?;

        let patch_hash = FileHandler::compute_md5(&patch_path).with_context(|| {
            format!("Failed to compute MD5 for patch: {}", patch_path.display())
        })?;
        log::debug!("[{region}] {patch_name} hex={patch_hash}");

        let base_data = Self::read_file(base_path)?;
        let patch_data = Self::read_file(&patch_path)?;

        let merged = apply_bsdiff(&base_data, &patch_data, patch_name)?;
        std::fs::write(output_path, &merged)
            .with_context(|| format!("Failed to write merged result: {}", output_path.display()))?;

        if let Some(expected) = &expected_hash {
            log::debug!("[{region}] {file_name} merged hex={expected}");
            if !Self::verify_hash(region, output_path, expected, Some(patch_name)) {
                return Err(AppError::message(format!(
                    "Hash mismatch after merge: {patch_name}"
                )));
            }
        } else {
            log::warn!("[{region}] merged {patch_name} without hash verification");
        }

        Ok(())
    }

    /// 返回是否真的写了文件 false 表示本地产物已是期望状态 直接跳过
    fn copy_original_file(&self, region: &Region, file_name: &str) -> Result<bool> {
        let output_path = self.unpacked_path(region, file_name);
        let expected_hash = self.expected_original_hash(region, file_name)?;

        if output_path.is_file() {
            log::debug!("[{region}] {file_name} already exists, verifying expected hash");
            if Self::verify_hash(region, &output_path, &expected_hash, Some(file_name)) {
                return Ok(false);
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

        Ok(true)
    }

    fn verify_cached_output(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
        output_path: &Path,
    ) -> Result<bool> {
        let Some(expected_hash) = self.expected_resource_hash(region, file_name, patch_name)?
        else {
            return Ok(false);
        };

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
    ) -> Result<Option<String>> {
        let mani_path = self.update_mani_path(region, file_name, patch_name);

        if let Ok(hash) = self.read_resource_hash(region, &mani_path, file_name) {
            return Ok(Some(hash));
        }

        // 本地清单补丁里没有这个资源 通常是它属于旧客户端版本
        // 用补丁条目自身的版本与附加路径取回对应版本的清单再查
        match self.expected_hash_for_patch(region, file_name, patch_name) {
            Ok(Some(hash)) => {
                log::debug!("[{region}] {file_name} hash from version manifest");
                Ok(Some(hash))
            }
            Ok(None) => {
                log::warn!("[{region}] {file_name} absent from {}", mani_path.display());
                log::warn!("[{region}] skip hash verification ");
                Ok(None)
            }
            Err(err) => {
                log::warn!(
                    "[{region}] {file_name} absent from {} and version lookup failed: {err}",
                    mani_path.display()
                );
                log::warn!("[{region}] skip hash verification ");
                Ok(None)
            }
        }
    }

    /// 取回补丁所属波次的清单补丁 再从中查期望 hash
    fn expected_hash_for_patch(
        &self,
        region: &Region,
        file_name: &str,
        patch_name: &str,
    ) -> Result<Option<String>> {
        let entries = self.load_decoded_manifest(region)?;
        let Some(entry) = entries.get(patch_name) else {
            return Ok(None);
        };

        if entry.additional_path.is_empty() {
            return Ok(None);
        }

        let mani_name = format!(
            "{}.{}",
            self.mani_prefix(file_name, patch_name),
            self.root_mani_filename
        );
        let reader = self.remote_mani(region, entry.version, &entry.additional_path, &mani_name)?;

        Ok(reader
            .get_resource_hash_by_patch(file_name)
            .map(|hash| hash.to_string()))
    }

    fn load_decoded_manifest(&self, region: &Region) -> Result<Rc<ResourceEntries>> {
        if let Some(cached) = self.decoded_cache.borrow().get(region) {
            return Ok(Rc::clone(cached));
        }

        let path = self
            .manifest_output_dir
            .join(region.as_str())
            .join(&self.decoded_manifest_filename);
        let entries = FileHandler::read_json::<ResourceEntries>(&path)
            .with_context(|| format!("Failed to read decoded manifest: {}", path.display()))?;
        let entries = Rc::new(entries);

        self.decoded_cache
            .borrow_mut()
            .insert(region.clone(), Rc::clone(&entries));

        Ok(entries)
    }

    /// 下载 {server}/{route}/{version}/{additional_path}/{root_mani} 并缓存
    fn remote_mani(
        &self,
        region: &Region,
        version: u64,
        additional_path: &str,
        mani_name: &str,
    ) -> Result<Rc<ManiReader>> {
        let key = (
            region.clone(),
            version,
            format!("{additional_path}/{mani_name}"),
        );
        if let Some(cached) = self.remote_cache.borrow().get(&key) {
            return Ok(Rc::clone(cached));
        }

        let path = self
            .patch_source_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(version.to_string())
            .join(additional_path)
            .join(mani_name);

        if !path.is_file() {
            let url = self.remote_mani_url(region, version, additional_path, mani_name)?;
            let bytes = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(self.requester.get_bytes(&url))
            })
            .with_context(|| format!("Failed to fetch manifest: {url}"))?;

            FileHandler::write_bytes(&bytes, &path)?;
        }

        let reader = Rc::new(ManiReader::new(&path, Some(region))?);
        self.remote_cache
            .borrow_mut()
            .insert(key, Rc::clone(&reader));

        Ok(reader)
    }

    fn remote_mani_url(
        &self,
        region: &Region,
        version: u64,
        additional_path: &str,
        mani_name: &str,
    ) -> Result<String> {
        let server = self
            .servers
            .get(region)
            .ok_or_else(|| AppError::MissingServer(region.to_string()))?;

        Ok([
            server.trim_end_matches('/'),
            self.resource_route.trim_matches('/'),
            &version.to_string(),
            additional_path.trim_matches('/'),
            mani_name.trim_start_matches('/'),
        ]
        .join("/"))
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
            log::warn!("[{region}] no hash for {file_name}, use patch manifest");
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
        self.patch_source_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(patch_name)
    }

    fn unpacked_path(&self, region: &Region, patch_name: &str) -> PathBuf {
        self.unpack_output_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(patch_name)
    }

    fn mani_file_path(&self, region: &Region, prefix: &str) -> PathBuf {
        let filename = format!("{prefix}.{}", self.root_mani_filename);
        self.patch_source_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(filename)
    }

    fn update_mani_path(&self, region: &Region, file_name: &str, patch_name: &str) -> PathBuf {
        let mani_prefix = self.mani_prefix(file_name, patch_name);

        self.mani_file_path(region, &mani_prefix)
    }

    fn mani_prefix(&self, file_name: &str, patch_name: &str) -> String {
        let suffix = format!(".{file_name}");
        let patch = patch_name.strip_suffix(&suffix).unwrap_or(patch_name);
        let stem = patch.rsplit_once('_').map_or(patch, |(prefix, _)| prefix);

        format!("{stem}{}", self.update_manifest_patch_tag)
    }

    fn add_mani_path(&self, region: &Region) -> PathBuf {
        let filename = if self.use_resource_regex {
            &self.root_mani_filename
        } else {
            &self.add_manifest_filename
        };

        self.patch_source_dir
            .join(region.as_str())
            .join(&self.platform)
            .join(filename)
    }

    fn load_patch_manifest(&self, region: &Region) -> Result<Rc<ResourceEntries>> {
        if let Some(cached) = self.manifest_cache.borrow().get(region) {
            return Ok(Rc::clone(cached));
        }

        let path = self.patch_manifest_path(region);
        let entries = FileHandler::read_json::<ResourceEntries>(&path)
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
                log::info!("[{region}] hash ok: {label}");
                true
            }
            Some(false) => {
                let actual = FileHandler::compute_md5(path).unwrap_or_default();
                log::warn!("[{region}] hash mismatch: {label} actual={actual} want={expected}");
                false
            }
            None => {
                if !path.is_file() {
                    log::warn!("[{region}] missing file: {}", path.display());
                } else {
                    log::error!("[{region}] md5 failed: {}", path.display());
                }
                false
            }
        }
    }
}

pub fn apply_bsdiff(base: &[u8], patch: &[u8], patch_name: &str) -> Result<Vec<u8>> {
    let patcher =
        Bspatch::new(patch).with_context(|| format!("Invalid bsdiff patch: {patch_name}"))?;

    let mut output = Vec::with_capacity(patcher.hint_target_size() as usize);
    patcher
        .apply(base, Cursor::new(&mut output))
        .with_context(|| format!("bsdiff apply failed: {patch_name}"))?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::apply_bsdiff;
    use qbsdiff::Bsdiff;

    fn patch_of(base: &[u8], target: &[u8]) -> Vec<u8> {
        let mut patch = Vec::new();
        Bsdiff::new(base, target)
            .compare(std::io::Cursor::new(&mut patch))
            .expect("qbsdiff should produce a patch");
        patch
    }

    #[test]
    fn roundtrips_identical_data() {
        let base = b"stella sora resource bundle payload".repeat(64);

        let merged = apply_bsdiff(&base, &patch_of(&base, &base), "p_1_u.same.bin").unwrap();

        assert_eq!(merged, base);
    }

    #[test]
    fn roundtrips_appended_tail() {
        let base = b"original payload".repeat(32);
        let mut target = base.clone();
        target.extend_from_slice(b"brand new tail bytes");

        let merged = apply_bsdiff(&base, &patch_of(&base, &target), "p_2_u.tail.bin").unwrap();

        assert_eq!(merged, target);
    }

    #[test]
    fn roundtrips_inserted_middle() {
        let base: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let mut target = base[..2000].to_vec();
        target.extend_from_slice(b"INSERTED-BLOCK-IN-THE-MIDDLE");
        target.extend_from_slice(&base[2000..]);

        let merged = apply_bsdiff(&base, &patch_of(&base, &target), "p_3_u.mid.bin").unwrap();

        assert_eq!(merged, target);
    }

    #[test]
    fn roundtrips_shrunk_output() {
        let base = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".repeat(128);
        let target = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".repeat(4);

        let merged = apply_bsdiff(&base, &patch_of(&base, &target), "p_4_u.shrink.bin").unwrap();

        assert_eq!(merged, target);
    }

    #[test]
    fn roundtrips_empty_to_payload() {
        let target = b"grown from nothing".repeat(16);

        let merged = apply_bsdiff(b"", &patch_of(b"", &target), "p_5_u.grow.bin").unwrap();

        assert_eq!(merged, target);
    }

    #[test]
    fn roundtrips_payload_to_empty() {
        let base = b"shrinks to nothing".repeat(16);

        let merged = apply_bsdiff(&base, &patch_of(&base, b""), "p_6_u.vanish.bin").unwrap();

        assert!(merged.is_empty());
    }

    #[test]
    fn roundtrips_large_binary_payload() {
        let base: Vec<u8> = (0..=255u8).cycle().take(3 * 1024 * 1024).collect();
        let mut target = base.clone();
        target[1024..1100].copy_from_slice(&[0xAB; 76]);
        target.extend_from_slice(&base[..512]);

        let merged = apply_bsdiff(&base, &patch_of(&base, &target), "p_7_u.big.bin").unwrap();

        assert_eq!(merged.len(), target.len());
        assert_eq!(merged, target);
    }

    #[test]
    fn rejects_corrupted_patch() {
        let base = b"base payload for corruption test".repeat(8);
        let mut patch = patch_of(&base, &b"totally different target".repeat(8));
        let last = patch.len() - 1;
        patch[last] ^= 0xFF;

        let err = apply_bsdiff(&base, &patch, "p_8_u.broken.bin").unwrap_err();

        assert!(err.to_string().contains("bsdiff apply failed"), "{err}");
    }

    #[test]
    fn rejects_garbage_patch() {
        let err =
            apply_bsdiff(b"anything", b"not a bsdiff patch at all", "p_9_u.junk.bin").unwrap_err();

        assert!(err.to_string().contains("Invalid bsdiff patch"), "{err}");
    }

    #[test]
    fn rejects_empty_patch() {
        let err = apply_bsdiff(b"base", b"", "p_10_u.empty.bin").unwrap_err();

        assert!(err.to_string().contains("Invalid bsdiff patch"), "{err}");
    }

    #[test]
    fn error_message_carries_patch_name() {
        let err = apply_bsdiff(b"base", b"\x00\x01\x02", "p_11_u.named.bin").unwrap_err();

        assert!(err.to_string().contains("p_11_u.named.bin"), "{err}");
    }

    const REAL_PATCH: &[u8] = include_bytes!("../tests/fixtures/p_7_u.icon-d.unity3d");

    #[test]
    fn parses_real_game_patch_header() {
        let patcher = qbsdiff::Bspatch::new(REAL_PATCH)
            .expect("real game patch should be a valid bsdiff stream");

        let target = patcher.hint_target_size();

        assert!(target > 0, "target size should be positive, got {target}");
        assert!(
            target < 512 * 1024 * 1024,
            "target size unreasonably large: {target}"
        );
    }

    #[test]
    fn real_patch_magic_matches_bsdiff40() {
        assert_eq!(&REAL_PATCH[..8], b"BSDIFF40");
        assert!(
            REAL_PATCH.len() > 32,
            "patch should carry a header plus blocks"
        );
    }
}
