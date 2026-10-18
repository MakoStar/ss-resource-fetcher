use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use file_utils::FileHandler;
use indexmap::{IndexMap, IndexSet};
use network_manager::HttpFetcher;

use crate::config::{
    AppConfig, ClientRegistryConfig, DELTA_PATCH_TAG, MANIFEST_PATCH_TAG, NEW_FILE_PATCH_TAG,
};
use crate::error::{AppError, Result};
use crate::generated::FileDiff;
use crate::manifest::{ManiReader, ManiResource, ManifestDecoder, ManifestDecryptor};
use crate::merger::MergeStats;
use crate::model::{PatchRef, Region, ResourceEntries, ResourceEntry, apply_order, tier_label};
use crate::network::{FetchStats, ManifestFetcher, ResourcesFetcher};
use crate::storage::{RegionFileStore, push_platform};
use crate::uncensor::merge::UncensorMerger;

pub struct RegionTarget {
    /// 实际下载资源所用的官方区域
    region: Region,
    /// 反和谐最终资源输出目录
    output_dir: PathBuf,
    /// 资源注册表 用于解析补丁名里的资源名
    registry: ClientRegistryConfig,
    /// 清单下载器
    manifest_fetcher: ManifestFetcher,
    /// 清单解密器
    decryptor: ManifestDecryptor,
    /// 补丁下载器 落到 Metadata 与 run 同一套下载逻辑
    patch_fetcher: ResourcesFetcher,
    /// 清单补丁下载器 落到 Metadata 可跨运行复用
    mani_fetcher: ResourcesFetcher,
    /// 下载的补丁资源目录 Uncensor/Metadata
    metadata_dir: PathBuf,
    /// 补丁合并器
    merger: UncensorMerger,
    /// 是否强制覆盖已存在的资源
    overwrite: bool,
    /// 是否保存清单 json
    save_manifest: bool,
    /// 解码清单存储
    resource_store: RegionFileStore,
    /// 补丁清单存储
    patch_store: RegionFileStore,
    /// 当前平台 落盘按平台分目录
    platform: String,
}

struct UncensorJob {
    /// 最终输出的资源名
    file: String,
    /// 本体条目 清单里没有则为 None
    base: Option<ResourceEntry>,
    /// 按版本升序的生效补丁名
    patches: Vec<String>,
}

struct UncensorPlan {
    /// 最终输出的资源名
    file: String,
    /// 本体名 整文件补丁可替代本体时为 None
    base: Option<String>,
    /// 按版本升序的补丁名
    steps: Vec<String>,
}

impl RegionTarget {
    pub fn new(
        app: &AppConfig,
        region: &Region,
        requester: &HttpFetcher,
        save_manifest: bool,
    ) -> Result<Self> {
        let server = app.server_config(region).ok_or_else(|| {
            AppError::message(format!(
                "uncensor region '{region}' has no server config: neither present in [SERVERS] nor a built-in region"
            ))
        })?;

        let servers: IndexMap<_, _> = [(region.clone(), server.url.clone())].into_iter().collect();
        let keys: IndexMap<_, _> = [(region.clone(), server.key.clone())].into_iter().collect();
        let platform = app.global.default_platform.clone();
        let output_dir = PathBuf::from(&app.task.uncensor.filepath.resource);
        let manifest_dir = &app.task.uncensor.filepath.manifest;
        let metadata_dir = PathBuf::from(&app.task.uncensor.filepath.metadata);

        Ok(Self {
            region: region.clone(),
            output_dir,
            registry: app.task.client.registry.clone(),
            manifest_fetcher: ManifestFetcher::new(
                app.task.client.route.manifest.clone(),
                servers.clone(),
                requester.clone(),
            ),
            decryptor: ManifestDecryptor::new(keys),
            patch_fetcher: ResourcesFetcher::new(
                app.task.client.route.resource.clone(),
                metadata_dir.clone(),
                platform.clone(),
                servers.clone(),
                app.task.client.flags.op.overwrite_resource,
                requester.clone(),
            ),
            mani_fetcher: ResourcesFetcher::new(
                app.task.client.route.resource.clone(),
                metadata_dir.clone(),
                platform.clone(),
                servers,
                app.task.client.flags.op.overwrite_resource,
                requester.clone(),
            ),
            metadata_dir,
            merger: UncensorMerger::new(app.task.client.registry.clone()),
            overwrite: app.task.client.flags.op.overwrite_resource,
            save_manifest,
            resource_store: RegionFileStore::new(
                manifest_dir,
                &app.task.client.filename.decoded_manifest,
            )
            .with_platform(&platform),
            patch_store: RegionFileStore::new(
                manifest_dir,
                &app.task.client.filename.patch_manifest,
            )
            .with_platform(&platform),
            platform,
        })
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub fn region(&self) -> &Region {
        &self.region
    }

    pub async fn download(&self, wanted: &IndexMap<String, ManiResource>) -> Result<()> {
        let hashes = wanted
            .iter()
            .map(|(name, res)| (name.clone(), res.hash.clone()))
            .collect();

        let names = wanted.keys().cloned().collect();
        self.download_set(&names, &hashes).await
    }

    pub async fn download_names(&self, wanted: &IndexSet<String>) -> Result<()> {
        self.download_set(wanted, &IndexMap::new()).await
    }

    async fn download_set(
        &self,
        wanted: &IndexSet<String>,
        hashes: &IndexMap<String, String>,
    ) -> Result<()> {
        let diffs = self.fetch_diffs().await?;
        let jobs = Self::plan_jobs(&diffs, wanted, &self.registry);

        let matched = jobs
            .iter()
            .filter(|job| job.base.is_some() || !job.patches.is_empty())
            .count();

        for job in &jobs {
            if job.base.is_none() && job.patches.is_empty() {
                log::warn!(
                    "[{}] uncensor resource not found in official manifest: {}",
                    self.region,
                    job.file
                );
            }
        }

        logger::step!("extract patch");
        log::info!(
            "[{}] matched {} of {} uncensor resources",
            self.region,
            matched,
            wanted.len()
        );
        let (entries, plans) = self.build_plans(&jobs, &diffs)?;
        if plans.is_empty() {
            return Err(AppError::message(format!(
                "none of the {} uncensor resources were found in region '{}' manifest",
                wanted.len(),
                self.region,
            )));
        }

        if self.save_manifest {
            self.patch_store.save_json(&self.region, &entries)?;
        }

        if self.overwrite {
            logger::tips!("overwrite enabled, resources re-download");
        }

        let mani = self.latest_mani(&diffs).await?;
        let expected = self.expected_hashes(&diffs, &plans, hashes, &mani)?;

        logger::head!("Fetch resources {}", self.region);
        let fetch_stats = self.fetch_all(&diffs, &plans).await?;
        log::info!(
            "[{}] {} file(s), {} new, {} cached, {:.2}MB",
            self.region,
            fetch_stats.total(),
            fetch_stats.downloaded,
            fetch_stats.cached,
            fetch_stats.megabytes()
        );

        logger::head!("Merge patches {}", self.region);
        let merge_stats = self.merge_all(&plans, &expected)?;

        logger::tips!(
            "[{}] uncensor pack done, {} file(s), {} merged, {} copied, {} cached",
            self.region,
            plans.len(),
            merge_stats.merged,
            merge_stats.copied,
            merge_stats.cached
        );

        Ok(())
    }

    async fn fetch_all(&self, diffs: &[FileDiff], plans: &[UncensorPlan]) -> Result<FetchStats> {
        let refs: Vec<_> = plans.iter().collect();
        let entries = self.entries_for(diffs, &refs);
        self.fetch_tiers(&entries).await
    }

    async fn fetch_tiers(&self, entries: &ResourceEntries) -> Result<FetchStats> {
        let mut tiers: BTreeMap<u64, Vec<(&str, &ResourceEntry)>> = BTreeMap::new();

        for (name, entry) in entries {
            tiers
                .entry(self.registry.patch_sequence(name))
                .or_default()
                .push((name.as_str(), entry));
        }

        let mut stats = FetchStats::default();

        for (tier, group) in &tiers {
            let fetched = self
                .patch_fetcher
                .fetch_group(&self.region, Some(*tier), group)
                .await?;
            stats.add(&fetched);
        }

        Ok(stats)
    }

    fn merge_all(
        &self,
        plans: &[UncensorPlan],
        expected: &IndexMap<String, String>,
    ) -> Result<MergeStats> {
        let refs: Vec<_> = plans.iter().collect();
        let mut stats = MergeStats::default();

        for (tier, group) in Self::merge_tiers(&self.registry, &refs) {
            logger::tier!("merge {} - {} job(s)", tier_label(tier), group.len());

            for plan in group {
                let output = self.output_path(&plan.file);

                if !self.overwrite && self.is_cached(&output, expected.get(&plan.file)) {
                    logger::step!("[{}] cached: {}", self.region, plan.file);
                    stats.cached += 1;
                    continue;
                }

                if plan.steps.is_empty() {
                    logger::step!("[{}] copy: {}", self.region, plan.file);
                } else {
                    logger::step!("[{}] merge: {}", self.region, plan.file);
                }

                self.merge_one(plan)?;

                if plan.steps.is_empty() {
                    stats.copied += 1;
                } else {
                    stats.merged += 1;
                }

                self.verify_output(&output, expected.get(&plan.file), &plan.file);
            }
        }

        log::info!(
            "[{}] {} file(s), {} merged, {} copied, {} cached",
            self.region,
            stats.total(),
            stats.merged,
            stats.copied,
            stats.cached
        );

        Ok(stats)
    }

    fn merge_tiers<'a>(
        registry: &ClientRegistryConfig,
        plans: &[&'a UncensorPlan],
    ) -> Vec<(u64, Vec<&'a UncensorPlan>)> {
        let mut tiers: BTreeMap<u64, Vec<&'a UncensorPlan>> = BTreeMap::new();

        for plan in plans {
            let tier = plan
                .steps
                .last()
                .map(|name| registry.patch_sequence(name))
                .unwrap_or_default();

            tiers.entry(tier).or_default().push(*plan);
        }

        tiers.into_iter().collect()
    }

    fn merge_one(&self, plan: &UncensorPlan) -> Result<()> {
        let base = plan.base.as_ref().map(|name| self.patch_path(name));
        let steps: Vec<(&str, PathBuf)> = plan
            .steps
            .iter()
            .map(|name| (name.as_str(), self.patch_path(name)))
            .collect();
        let output = self.output_path(&plan.file);
        logger::test!("{}", output.display());
        self.merger
            .merge(&self.region, &plan.file, base.as_deref(), &steps, &output)
    }

    fn expected_hashes(
        &self,
        diffs: &[FileDiff],
        plans: &[UncensorPlan],
        hashes: &IndexMap<String, String>,
        mani: &Option<ManiReader>,
    ) -> Result<IndexMap<String, String>> {
        let mut expected = hashes.clone();
        let by_name: HashMap<&str, _> = diffs
            .iter()
            .map(|diff| (diff.file_name.as_str(), diff))
            .collect();

        for plan in plans {
            if !plan.steps.is_empty() {
                continue;
            }

            let base = plan
                .base
                .as_ref()
                .and_then(|name| by_name.get(name.as_str()));
            if let Some(diff) = base {
                expected.insert(plan.file.clone(), diff.hash.clone());
            }
        }

        let Some(mani) = mani else {
            return Ok(expected);
        };

        for plan in plans {
            let Some(hash) = mani.get_resource_hash_by_patch(&plan.file) else {
                log::debug!(
                    "[{}] {} missing from latest mani, keep fallback hash",
                    self.region,
                    plan.file
                );
                continue;
            };

            if let Some(third) = expected.get(&plan.file) {
                if !third.eq_ignore_ascii_case(hash) {
                    log::debug!(
                        "[{}] {} uncensor hash differs from mani hash",
                        self.region,
                        plan.file
                    );
                }
            }

            expected.insert(plan.file.clone(), hash.to_string());
        }

        Ok(expected)
    }

    fn pick_mani<'a>(
        registry: &ClientRegistryConfig,
        diffs: &'a [FileDiff],
    ) -> Option<&'a FileDiff> {
        diffs
            .iter()
            .filter(|diff| registry.patch_tag(&diff.file_name) == Some(MANIFEST_PATCH_TAG))
            .max_by_key(|diff| (diff.version, registry.patch_sequence(&diff.file_name)))
    }

    async fn latest_mani(&self, diffs: &[FileDiff]) -> Result<Option<ManiReader>> {
        let Some(diff) = Self::pick_mani(&self.registry, diffs) else {
            log::warn!("[{}] no manifest patch in resource manifest", self.region);
            return Ok(None);
        };

        logger::head!("Fetch manifest patch {}", self.region);
        logger::tips!(
            "[{}] latest manifest patch: {} (v{})",
            self.region,
            diff.file_name,
            diff.version
        );

        let entry = ResourceEntry::from(diff);
        if let Err(err) = self
            .mani_fetcher
            .fetch_group(&self.region, None, &[(diff.file_name.as_str(), &entry)])
            .await
        {
            log::warn!("[{}] manifest patch unavailable: {err}", self.region);
            return Ok(None);
        }

        let path = self.mani_path(&diff.file_name);

        Ok(Some(ManiReader::new(&path, Some(&self.region))?))
    }

    fn verify_output(&self, output: &Path, expected: Option<&String>, file: &str) -> bool {
        let Some(expected) = expected else {
            log::debug!(
                "[{}] {} no expected hash, verify skipped",
                self.region,
                file
            );
            return true;
        };

        match FileHandler::verify_md5(output, expected) {
            Ok(()) => {
                log::info!("[{}] hash ok: {}", self.region, file);
                true
            }
            Err(err) => {
                log::warn!("[{}] hash mismatch: {} ({err})", self.region, file);
                false
            }
        }
    }

    async fn fetch_diffs(&self) -> Result<Vec<FileDiff>> {
        let raw = self.manifest_fetcher.fetch_one(&self.region).await?;

        logger::step!("decode manifest");
        let decrypted = self.decryptor.decrypt_one(&self.region, &raw)?;
        let diffs = ManifestDecoder::decode_one(&self.region, &decrypted)?;

        if self.save_manifest {
            self.resource_store
                .save_json(&self.region, &ManifestDecoder::json_view(&diffs))?;
        }

        Ok(diffs)
    }

    fn plan_jobs(
        diffs: &[FileDiff],
        wanted: &IndexSet<String>,
        registry: &ClientRegistryConfig,
    ) -> Vec<UncensorJob> {
        let mut bases = HashMap::new();
        let mut groups: HashMap<&str, Vec<_>> = HashMap::new();

        for diff in diffs {
            let name = diff.file_name.as_str();

            if wanted.contains(name) {
                bases.insert(name, diff);
                continue;
            }

            let Some(base) = registry.patch_base_name(name) else {
                continue;
            };
            if !wanted.contains(base) {
                continue;
            }
            if !matches!(
                registry.patch_tag(name),
                Some(NEW_FILE_PATCH_TAG) | Some(DELTA_PATCH_TAG)
            ) {
                continue;
            }

            groups.entry(base).or_default().push(PatchRef::new(
                registry,
                name,
                diff.version as u64,
            ));
        }

        wanted
            .iter()
            .map(|file| {
                let base = bases
                    .get(file.as_str())
                    .map(|diff| ResourceEntry::from(*diff));
                let patches = groups.get(file.as_str()).map(Vec::as_slice).unwrap_or(&[]);
                let kept = apply_order(base.as_ref().map(|entry| entry.version), patches, |name| {
                    registry.is_new_file_patch(name)
                });

                UncensorJob {
                    file: file.clone(),
                    base,
                    patches: kept.into_iter().map(str::to_string).collect(),
                }
            })
            .collect()
    }

    fn build_plans(
        &self,
        jobs: &[UncensorJob],
        diffs: &[FileDiff],
    ) -> Result<(ResourceEntries, Vec<UncensorPlan>)> {
        let mut plans = Vec::new();
        let mut skipped = 0;

        for job in jobs {
            let Some(plan) = self.build_plan(job) else {
                skipped += 1;
                continue;
            };

            plans.push(plan);
        }

        if skipped > 0 {
            log::info!(
                "[{}] {} resource(s) skipped, no base to merge",
                self.region,
                skipped
            );
        }

        let refs: Vec<_> = plans.iter().collect();
        let entries = self.entries_for(diffs, &refs);

        Ok((entries, plans))
    }

    fn build_plan(&self, job: &UncensorJob) -> Option<UncensorPlan> {
        let replaced = job
            .patches
            .first()
            .is_some_and(|name| self.registry.is_new_file_patch(name));

        let base = if replaced {
            None
        } else {
            job.base.as_ref().map(|entry| entry.file_name.clone())
        };

        log::debug!(
            "[{}] {} base={:?} steps={:?}",
            self.region,
            job.file,
            base,
            job.patches
        );

        if base.is_none() && !replaced {
            log::warn!(
                "[{}] {} has delta patch without base, skipped",
                self.region,
                job.file
            );
            return None;
        }

        Some(UncensorPlan {
            file: job.file.clone(),
            base,
            steps: job.patches.clone(),
        })
    }

    fn entries_for(&self, diffs: &[FileDiff], plans: &[&UncensorPlan]) -> ResourceEntries {
        let by_name: HashMap<&str, _> = diffs
            .iter()
            .map(|diff| (diff.file_name.as_str(), diff))
            .collect();

        let mut entries = ResourceEntries::new();

        for plan in plans {
            if let Some(base) = &plan.base {
                if let Some(diff) = by_name.get(base.as_str()) {
                    entries.insert(base.clone(), ResourceEntry::from(*diff));
                }
            }

            for step in &plan.steps {
                if let Some(diff) = by_name.get(step.as_str()) {
                    entries.insert(step.clone(), ResourceEntry::from(*diff));
                }
            }
        }

        entries
    }

    fn is_cached(&self, output: &Path, hash: Option<&String>) -> bool {
        if self.overwrite {
            return false;
        }

        let Some(hash) = hash else {
            return false;
        };

        output.is_file() && FileHandler::verify_md5(output, hash).is_ok()
    }

    fn patch_dir(&self) -> PathBuf {
        push_platform(self.metadata_dir.join(self.region.as_str()), &self.platform)
    }

    fn mani_path(&self, name: &str) -> PathBuf {
        self.patch_dir().join(name)
    }

    fn patch_path(&self, name: &str) -> PathBuf {
        self.patch_dir().join(name)
    }

    fn output_path(&self, file: &str) -> PathBuf {
        push_platform(self.output_dir.join(self.region.as_str()), &self.platform).join(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(file_name: &str, version: i64) -> FileDiff {
        FileDiff {
            file_name: file_name.to_string(),
            hash: String::new(),
            version,
            additional_path: String::new(),
        }
    }

    fn wanted(names: &[&str]) -> IndexSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn registry() -> ClientRegistryConfig {
        AppConfig::default().task.client.registry
    }

    fn job_of<'a>(jobs: &'a [UncensorJob], file: &str) -> &'a UncensorJob {
        jobs.iter()
            .find(|job| job.file == file)
            .expect("missing job")
    }

    #[test]
    fn patch_base_name_keeps_resource_name() {
        let registry = registry();

        assert_eq!(
            registry.patch_base_name("p_4_n.disc_4057.unity3d"),
            Some("disc_4057.unity3d")
        );
        assert_eq!(
            registry.patch_base_name("p_12_n.char_2d_13902.unity3d"),
            Some("char_2d_13902.unity3d")
        );
        assert_eq!(registry.patch_base_name("disc_4057.unity3d"), None);
        assert_eq!(registry.patch_base_name("p_4_n"), None);
    }

    #[test]
    fn keeps_base_and_newer_delta_patches() {
        let diffs = vec![
            diff("actor_l2d_common.unity3d", 124),
            diff("p_6_u.actor_l2d_common.unity3d", 108),
            diff("p_8_u.actor_l2d_common.unity3d", 133),
            diff("p_12_u.actor_l2d_common.unity3d", 114),
            diff("unrelated.unity3d", 1),
        ];

        let jobs =
            RegionTarget::plan_jobs(&diffs, &wanted(&["actor_l2d_common.unity3d"]), &registry());
        let job = job_of(&jobs, "actor_l2d_common.unity3d");

        assert!(job.base.is_some());
        assert_eq!(job.patches, vec!["p_8_u.actor_l2d_common.unity3d"]);
    }

    #[test]
    fn new_file_patch_replaces_the_base() {
        let diffs = vec![
            diff("char_2d_13902.unity3d", 91),
            diff("p_11_n.char_2d_13902.unity3d", 137),
            diff("p_13_u.char_2d_13902.unity3d", 148),
        ];

        let jobs =
            RegionTarget::plan_jobs(&diffs, &wanted(&["char_2d_13902.unity3d"]), &registry());
        let job = job_of(&jobs, "char_2d_13902.unity3d");

        assert_eq!(
            job.patches,
            vec![
                "p_11_n.char_2d_13902.unity3d",
                "p_13_u.char_2d_13902.unity3d"
            ]
        );
    }

    #[test]
    fn uncensor_names_fall_back_to_patch_entries() {
        let diffs = vec![
            diff("p_11_n.char_2d_13902.unity3d", 6),
            diff("p_12_n.char_2d_13902.unity3d", 7),
        ];

        let jobs =
            RegionTarget::plan_jobs(&diffs, &wanted(&["char_2d_13902.unity3d"]), &registry());
        let job = job_of(&jobs, "char_2d_13902.unity3d");

        assert!(job.base.is_none());
        assert_eq!(job.patches, vec!["p_12_n.char_2d_13902.unity3d"]);
    }

    fn plan(file: &str, base: Option<&str>, steps: &[&str]) -> UncensorPlan {
        UncensorPlan {
            file: file.to_string(),
            base: base.map(str::to_string),
            steps: steps.iter().map(|step| step.to_string()).collect(),
        }
    }

    #[test]
    fn picks_manifest_patch_with_highest_version() {
        let diffs = vec![
            diff("p_21_m.ss_win.mani", 133),
            diff("p_13_m.ss_win.mani", 148),
            diff("p_12_m.ss_win.mani", 147),
            diff("win.unity3d", 134),
        ];

        let picked = RegionTarget::pick_mani(&registry(), &diffs).expect("missing mani");

        assert_eq!(picked.file_name, "p_13_m.ss_win.mani");
    }

    #[test]
    fn picks_nothing_without_manifest_patch() {
        let diffs = vec![diff("win.unity3d", 134)];

        assert!(RegionTarget::pick_mani(&registry(), &diffs).is_none());
    }

    #[test]
    fn merge_jobs_group_by_final_patch_sequence() {
        let plans = [
            plan("no_patch.unity3d", Some("no_patch.unity3d"), &[]),
            plan(
                "delta.unity3d",
                Some("delta.unity3d"),
                &["p_8_u.delta.unity3d"],
            ),
            plan(
                "chain.unity3d",
                None,
                &["p_8_n.chain.unity3d", "p_12_u.chain.unity3d"],
            ),
        ];

        let refs: Vec<_> = plans.iter().collect();
        let tiers = RegionTarget::merge_tiers(&registry(), &refs);

        assert_eq!(
            tiers
                .iter()
                .map(|(tier, group)| (*tier, group.len()))
                .collect::<Vec<_>>(),
            vec![(0, 1), (8, 1), (12, 1)]
        );
    }

    #[test]
    fn uncensor_names_outside_manifest_stay_missing() {
        let diffs = vec![diff("p_4_n.disc_4057.unity3d", 4)];

        let jobs =
            RegionTarget::plan_jobs(&diffs, &wanted(&["char_2d_13902.unity3d"]), &registry());
        let job = job_of(&jobs, "char_2d_13902.unity3d");

        assert!(job.base.is_none());
        assert!(job.patches.is_empty());
    }

    #[test]
    fn paths_carry_platform_layer() {
        let app = AppConfig::default();
        let requester = HttpFetcher::new(&app.build_fetcher_config());
        let target = RegionTarget::new(&app, &Region::new("CN"), &requester, false).unwrap();

        let patch = target.patch_dir();
        assert!(patch.ends_with("Metadata/CN/win"), "{}", patch.display());

        let output = target.output_path("data.unity3d");
        assert!(
            output.ends_with("Resource/CN/win/data.unity3d"),
            "{}",
            output.display()
        );

        let mani = target.mani_path("p_13_m.ss_win.mani");
        assert!(
            mani.ends_with("Metadata/CN/win/p_13_m.ss_win.mani"),
            "{}",
            mani.display()
        );
    }
}
