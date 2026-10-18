use std::collections::BTreeMap;
use std::path::Path;

use file_utils::FileHandler;
use serde_json::{Map, Value};

use crate::config::{AppConfig, ClientRegistryConfig, default_region_filtered};
use crate::error::{AppError, Result};
use crate::extractor::{GameVersionExtractor, PatchManifestExtractor, PatchVersionExtractor};
use crate::manifest::{ManifestDecoder, ManifestDecryptor};
use crate::merger::{MergeStats, PatchMerger};
use crate::model::{
    LauncherFilter, LauncherSelect, LauncherSource, Region, RegionBytes, RegionFileDiffs,
    RegionResources, ResourceEntries, ResourceEntry, UncensorSource,
};
use crate::network::{
    FetchStats, LauncherFetcher, ManifestFetcher, ResourcesFetcher, UncensorPatchFetcher,
};
use crate::storage::RegionFileStore;

#[derive(Debug, Clone)]
pub enum TaskOptions {
    /// 常规资源流水线
    Pipeline {
        /// 命令行是否强制开关过时补丁过滤
        filter_outdated: Option<bool>,
    },
    /// 只生成清单记录
    Record,
    /// 只下载 launcher 资源包
    Launcher(LauncherTask),
    /// 只下载反和谐资源包
    Uncensor {
        /// 反和谐资源包的下载来源
        source: UncensorSource,
        /// 是否保存资源与补丁清单 json
        save_manifest: bool,
    },
}

impl Default for TaskOptions {
    fn default() -> Self {
        Self::Pipeline {
            filter_outdated: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LauncherTask {
    /// 下载区域 未指定则为全部区域
    pub source: LauncherSource,
    /// 是否下载清单里的全部资源
    pub all: bool,
    /// 是否启用正则筛选
    pub regex: Option<bool>,
    /// 命令行传入的正则与文件列表
    pub filter: LauncherFilter,
    /// 是否按清单里的 path 建目录保存
    pub keep: bool,
}

pub struct ResourcePipeline {
    /// 全局配置
    config: &'static AppConfig,
    /// 原始清单存储
    raw_manifest_store: RegionFileStore,
    /// 解密清单存储
    decrypted_manifest_store: RegionFileStore,
    /// 解码清单存储
    decoded_manifest_store: RegionFileStore,
    /// 补丁清单存储
    patch_manifest_store: RegionFileStore,
}

impl ResourcePipeline {
    pub fn new(config: &'static AppConfig) -> Self {
        let manifest_dir = &config.task.client.filepath.manifest;
        let names = &config.task.client.filename;

        Self {
            config,
            raw_manifest_store: RegionFileStore::new(manifest_dir, &names.raw_manifest),
            decrypted_manifest_store: RegionFileStore::new(manifest_dir, &names.decrypted_manifest),
            decoded_manifest_store: RegionFileStore::new(manifest_dir, &names.decoded_manifest),
            patch_manifest_store: RegionFileStore::new(manifest_dir, &names.patch_manifest),
        }
    }

    pub async fn run(&self, task: &TaskOptions) -> Result<()> {
        if default_region_filtered()
            && matches!(task, TaskOptions::Pipeline { .. } | TaskOptions::Record)
        {
            logger::tips!(
                "default region enabled, only '{}' kept",
                self.config.global.default_region
            );
        }

        match task {
            TaskOptions::Launcher(task) => self.download_launcher_pack(task).await,
            TaskOptions::Uncensor {
                source,
                save_manifest,
            } => self.download_uncensor_pack(source, *save_manifest).await,
            TaskOptions::Pipeline { filter_outdated } => {
                self.run_pipeline(false, *filter_outdated).await
            }
            TaskOptions::Record => self.run_pipeline(true, None).await,
        }
    }

    async fn run_pipeline(&self, record: bool, filter_outdated: Option<bool>) -> Result<()> {
        let flags = &self.config.task.client.flags;

        let raw = self.fetch_manifests().await?;
        let decrypted = self.decrypt_manifests(&raw)?;
        let decoded = self.decode_manifests(&decrypted)?;

        if !flags.op.extract_patch_manifest {
            logger::tips!("patch manifest extract disabled");
            return Ok(());
        }
        let patches = self.extract_patch_manifests(&decoded, record, filter_outdated)?;

        if !flags.op.download_resource {
            logger::tips!("resource download disabled");
            return Ok(());
        }
        self.download_and_merge(&patches, record).await?;

        if flags.save.version_file {
            self.save_version_file(&patches, record)?;
        }

        Ok(())
    }

    async fn download_uncensor_pack(
        &self,
        source: &UncensorSource,
        save_manifest: bool,
    ) -> Result<()> {
        UncensorPatchFetcher::from_app_config(self.config, source, save_manifest)?
            .fetch_all()
            .await
    }

    async fn download_launcher_pack(&self, task: &LauncherTask) -> Result<()> {
        self.report_launcher_regions(&task.source)?;

        let select = LauncherSelect::resolve(
            task.all,
            task.regex,
            task.filter.clone(),
            LauncherFilter {
                patterns: self.config.task.launcher.registry.res_patterns.clone(),
                files: self.config.task.launcher.registry.fetch_files.clone(),
            },
        )?;

        logger::tips!("launcher select mode: {}", select.label());

        LauncherFetcher::from_app_config(self.config)?
            .with_keep_path(task.keep)
            .with_select(&select)?
            .fetch_all(&task.source)
            .await
    }

    fn report_launcher_regions(&self, source: &LauncherSource) -> Result<()> {
        let configured: Vec<_> = self.config.task.launcher.region_names().collect();
        let in_config = |region: &Region| configured.contains(&region.as_str());

        match source {
            LauncherSource::Region(region) if in_config(region) => {
                logger::tips!("launcher region: {region}");
                Ok(())
            }
            LauncherSource::Region(region)
                if self.config.task.launcher.has_region(region.as_str()) =>
            {
                logger::tips!(
                    "launcher region: {region} (built-in endpoint, kept: {})",
                    configured.join(", ")
                );
                Ok(())
            }
            LauncherSource::Region(region) => Err(AppError::message(format!(
                "launcher region '{region}' has no endpoint. Available: {}",
                self.config
                    .task
                    .launcher
                    .builtin_region_names()
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
            LauncherSource::AllRegions => {
                logger::tips!("launcher regions: {} (-r to narrow)", configured.join(", "));
                Ok(())
            }
        }
    }

    async fn fetch_manifests(&self) -> Result<RegionBytes> {
        let fetcher = ManifestFetcher::from_app_config(self.config);
        let save = self.config.task.client.flags.save.raw_manifest;
        let mut manifests = RegionBytes::new();

        logger::head!(
            "Fetch manifest {}",
            region_list(self.config.regions().iter())
        );

        for region in self.config.regions() {
            let bytes = fetcher.fetch_one(&region).await?;
            if save {
                self.raw_manifest_store.save_bytes(&region, &bytes)?;
            }
            manifests.insert(region, bytes);
        }

        Ok(manifests)
    }

    fn decrypt_manifests(&self, raw: &RegionBytes) -> Result<RegionBytes> {
        let decryptor = ManifestDecryptor::from_app_config(self.config);
        let save = self.config.task.client.flags.save.decrypted_manifest;
        let mut decrypted = RegionBytes::new();

        logger::head!("Decrypt manifest {}", region_list(raw.keys()));

        for (region, bytes) in raw {
            let data = decryptor.decrypt_one(region, bytes)?;
            if save {
                self.decrypted_manifest_store.save_bytes(region, &data)?;
            }
            decrypted.insert(region.clone(), data);
        }

        Ok(decrypted)
    }

    fn decode_manifests(&self, decrypted: &RegionBytes) -> Result<RegionFileDiffs> {
        let save = self.config.task.client.flags.save.decoded_manifest;
        let mut decoded = RegionFileDiffs::new();

        logger::head!("Decode manifest {}", region_list(decrypted.keys()));

        for (region, bytes) in decrypted {
            let diffs = ManifestDecoder::decode_one(region, bytes)?;
            if save {
                self.decoded_manifest_store
                    .save_json(region, &ManifestDecoder::json_view(&diffs))?;
            }
            decoded.insert(region.clone(), diffs);
        }

        Ok(decoded)
    }

    fn extract_patch_manifests(
        &self,
        decoded: &RegionFileDiffs,
        record: bool,
        filter_outdated: Option<bool>,
    ) -> Result<RegionResources> {
        let extractor = PatchManifestExtractor::from_app_config(self.config)
            .with_filter_outdated(filter_outdated);
        let save = self.config.task.client.flags.save.patch_manifest;
        let mut patches = RegionResources::new();

        let title = if record {
            "Extract root manifest"
        } else {
            "Extract patch manifest"
        };
        logger::head!("{title} {}", region_list(decoded.keys()));

        for (region, diffs) in decoded {
            let entries = if record {
                extractor.extract_root_one(diffs)
            } else {
                extractor.extract_one(region, diffs)?
            };

            if save {
                self.patch_manifest_store.save_json(region, &entries)?;
            }
            patches.insert(region.clone(), entries);
        }

        Ok(patches)
    }

    async fn download_and_merge(&self, patches: &RegionResources, record: bool) -> Result<()> {
        let flags = &self.config.task.client.flags;

        if !record && !flags.op.merge_patch {
            logger::tips!("merge patch disabled");
        }
        if flags.op.overwrite_resource {
            logger::tips!("overwrite enabled, resources re-download");
        }

        let fetcher = ResourcesFetcher::from_app_config(self.config);
        let merger = PatchMerger::from_app_config(self.config);
        let do_merge = !record && flags.op.merge_patch;

        for (region, entries) in patches {
            logger::head!("Fetch resources {region}");
            log::debug!(
                "[{region}] download dir: {}/{}/{}",
                self.config.task.client.filepath.patch,
                region.as_str(),
                self.config.global.default_platform
            );

            let tiers = Self::tier_groups(&self.config.task.client.registry, entries);
            let stats = self.fetch_tiers(&fetcher, region, &tiers).await?;

            log::info!(
                "[{region}] {} file(s), {} new, {} cached, {:.2}MB",
                stats.total(),
                stats.downloaded,
                stats.cached,
                stats.megabytes()
            );

            if !do_merge {
                continue;
            }

            merger.report_tiers(region)?;
            logger::head!("Merge patches {region}");
            log::debug!(
                "[{region}] merge output: {}/{}/{}",
                self.config.task.client.filepath.unpack,
                region.as_str(),
                self.config.global.default_platform
            );

            let merged = self.merge_tiers(&merger, region, &tiers)?;

            log::info!(
                "[{region}] {} file(s), {} merged, {} copied, {} cached",
                merged.total(),
                merged.merged,
                merged.copied,
                merged.cached
            );
        }

        if record {
            logger::tips!("record only, merge skipped");
        }

        Ok(())
    }

    async fn fetch_tiers(
        &self,
        fetcher: &ResourcesFetcher,
        region: &Region,
        tiers: &[(u64, Vec<(&str, &ResourceEntry)>)],
    ) -> Result<FetchStats> {
        let mut stats = FetchStats::default();

        for (tier, group) in tiers {
            let tier_stats = fetcher.fetch_group(region, Some(*tier), group).await?;
            log::debug!(
                "[{region}] tier {tier}: {} new, {} cached, {:.2}MB",
                tier_stats.downloaded,
                tier_stats.cached,
                tier_stats.megabytes()
            );
            stats.add(&tier_stats);
        }

        Ok(stats)
    }

    fn merge_tiers(
        &self,
        merger: &PatchMerger,
        region: &Region,
        tiers: &[(u64, Vec<(&str, &ResourceEntry)>)],
    ) -> Result<MergeStats> {
        let mut stats = MergeStats::default();

        for (tier, _) in tiers {
            stats.add(&merger.apply_tier(region, *tier)?);
        }

        Ok(stats)
    }

    fn tier_groups<'a>(
        registry: &ClientRegistryConfig,
        entries: &'a ResourceEntries,
    ) -> Vec<(u64, Vec<(&'a str, &'a ResourceEntry)>)> {
        let mut tiers: BTreeMap<u64, Vec<(&str, &ResourceEntry)>> = BTreeMap::new();

        for (name, entry) in entries {
            let tier = registry.patch_sequence(name);
            tiers.entry(tier).or_default().push((name.as_str(), entry));
        }

        tiers.into_iter().collect()
    }

    fn save_version_file(&self, patches: &RegionResources, record: bool) -> Result<()> {
        let flags = &self.config.task.client.flags;

        let patch_version_json = if flags.op.extract_patch_version && !flags.op.use_resource_regex {
            let extractor = PatchVersionExtractor::from_app_config(self.config, patches);
            if record {
                extractor.extract_mani(&self.config.task.client.filename.root_mani)?
            } else {
                extractor.extract(self.config.task.client.registry.unpack_set())?
            }
        } else {
            Value::Object(Map::new())
        };

        let game_version_json =
            GameVersionExtractor::from_app_config(self.config).extract_regions()?;

        let mut merged = patch_version_json;
        deep_merge(&mut merged, Value::Object(game_version_json));
        self.write_version_file(&merged)?;

        Ok(())
    }

    fn write_version_file(&self, version: &Value) -> Result<()> {
        let path = Path::new(&self.config.task.client.filepath.versions)
            .join(&self.config.task.client.filename.version);

        FileHandler::write_json(version, &path)?;
        log::info!("saved version json: {}", path.display());

        Ok(())
    }
}

fn region_list<'a>(keys: impl Iterator<Item = &'a Region>) -> String {
    keys.map(Region::as_str).collect::<Vec<_>>().join(", ")
}

fn deep_merge(base: &mut Value, overlay: Value) {
    let Value::Object(overlay_map) = overlay else {
        return;
    };

    let Some(base_map) = base.as_object_mut() else {
        log::warn!("deep merge skipped: base value is not an object");
        return;
    };

    for (key, value) in overlay_map {
        match base_map.get_mut(&key) {
            Some(existing) if existing.is_object() => deep_merge(existing, value),
            _ => {
                base_map.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::deep_merge;
    use serde_json::json;

    #[test]
    fn merges_disjoint_keys() {
        let mut base = json!({"CN": {"patch_ver": "1"}});
        deep_merge(&mut base, json!({"JP": {"patch_ver": "2"}}));

        assert_eq!(
            base,
            json!({"CN": {"patch_ver": "1"}, "JP": {"patch_ver": "2"}})
        );
    }

    #[test]
    fn merges_nested_objects() {
        let mut base = json!({"CN": {"patch_ver": "1"}});
        deep_merge(&mut base, json!({"CN": {"game_ver": "9"}}));

        assert_eq!(base, json!({"CN": {"patch_ver": "1", "game_ver": "9"}}));
    }

    #[test]
    fn overlay_wins_on_conflict() {
        let mut base = json!({"CN": {"patch_ver": "1"}});
        deep_merge(&mut base, json!({"CN": {"patch_ver": "2"}}));

        assert_eq!(base, json!({"CN": {"patch_ver": "2"}}));
    }

    #[test]
    fn non_object_overlay_is_ignored() {
        let mut base = json!({"CN": {}});
        deep_merge(&mut base, json!("not-an-object"));

        assert_eq!(base, json!({"CN": {}}));
    }
}
