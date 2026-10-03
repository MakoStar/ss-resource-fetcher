use std::path::Path;

use file_utils::FileHandler;
use serde_json::{Map, Value};

use crate::config::{AppConfig, default_region_filtered};
use crate::error::{AppError, Result};
use crate::extractor::{GameVersionExtractor, PatchManifestExtractor, PatchVersionExtractor};
use crate::manifest::{ManifestDecoder, ManifestDecryptor};
use crate::model::{
    LauncherFilter, LauncherSelect, LauncherSource, Region, RegionBytes, RegionFileDiffs,
    RegionResources, UncensorSource,
};
use crate::network::{LauncherFetcher, ManifestFetcher, ResourcesFetcher, UncensorPatchFetcher};
use crate::patch::PatchMerger;
use crate::storage::RegionFileStore;

#[derive(Debug, Clone, Default)]
pub enum TaskOptions {
    /// 常规资源流水线
    #[default]
    Pipeline,
    /// 只生成清单记录
    Record,
    /// 只下载反和谐资源包
    Uncensor {
        /// 反和谐资源包的下载来源
        source: UncensorSource,
    },
    /// 只下载 launcher 资源包
    Launcher(LauncherTask),
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
        let manifest_dir = &config.file_path.manifest_output_dir;
        let names = &config.file_name;

        Self {
            config,
            raw_manifest_store: RegionFileStore::new(manifest_dir, &names.manifest_raw_file),
            decrypted_manifest_store: RegionFileStore::new(
                manifest_dir,
                &names.manifest_decrypt_file,
            ),
            decoded_manifest_store: RegionFileStore::new(
                manifest_dir,
                &names.manifest_decoded_file,
            ),
            patch_manifest_store: RegionFileStore::new(manifest_dir, &names.patch_manifest_file),
        }
    }

    pub async fn run(&self, task: &TaskOptions) -> Result<()> {
        if default_region_filtered() {
            logger::tips!(
                "IS_USE_DEFAULT_REGION is enabled, only '{}' region is kept !!!",
                self.config.default_region
            );
        }

        match task {
            TaskOptions::Uncensor { source } => self.download_uncensor_pack(source).await,
            TaskOptions::Launcher(task) => self.download_launcher_pack(task).await,
            TaskOptions::Pipeline => self.run_pipeline(false).await,
            TaskOptions::Record => self.run_pipeline(true).await,
        }
    }

    async fn run_pipeline(&self, record: bool) -> Result<()> {
        let flags = &self.config.feature_flags;

        let raw = self.fetch_manifests().await?;
        let decrypted = self.decrypt_manifests(&raw)?;
        let decoded = self.decode_manifests(&decrypted)?;

        if !flags.is_extract_patch_manifest {
            logger::tips!("FEATURE: IS_EXTRACT_PATCH_MANIFEST IS DISABLED.");
            return Ok(());
        }
        let patches = self.extract_patch_manifests(&decoded, record)?;

        if !flags.is_download_resource {
            logger::tips!("FEATURE: IS_DOWNLOAD_RESOURCE IS DISABLED.");
            return Ok(());
        }
        self.download_resources(&patches).await?;

        self.merge_patches(record)?;

        if flags.is_save_version_file {
            self.save_version_file(&patches, record)?;
        }

        Ok(())
    }

    async fn download_uncensor_pack(&self, source: &UncensorSource) -> Result<()> {
        UncensorPatchFetcher::from_app_config(self.config, source)?
            .fetch_all()
            .await
    }

    async fn download_launcher_pack(&self, task: &LauncherTask) -> Result<()> {
        self.report_launcher_regions(&task.source)?;

        let select: LauncherSelect = LauncherSelect::resolve(
            task.all,
            task.regex,
            task.filter.clone(),
            LauncherFilter {
                patterns: self.config.launcher.patterns.clone(),
                files: self.config.launcher.files.clone(),
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
        let configured: Vec<&str> = self.config.launcher.region_names().collect();
        let in_config = |region: &Region| configured.contains(&region.as_str());

        match source {
            LauncherSource::Region(region) if in_config(region) => {
                logger::tips!("launcher region: {region}");
                Ok(())
            }
            LauncherSource::Region(region) if self.config.launcher.has_region(region.as_str()) => {
                logger::tips!(
                    "launcher region: {region} (built-in endpoint, kept: {})",
                    configured.join(", ")
                );
                Ok(())
            }
            LauncherSource::Region(region) => Err(AppError::message(format!(
                "launcher region '{region}' has no endpoint. Available: {}",
                self.config
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
        let save = self.config.feature_flags.is_save_raw_manifest;
        let mut manifests = RegionBytes::new();

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
        let save = self.config.feature_flags.is_save_decrypted_manifest;
        let mut decrypted = RegionBytes::new();

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
        let save = self.config.feature_flags.is_save_decoded_manifest;
        let mut decoded = RegionFileDiffs::new();

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
    ) -> Result<RegionResources> {
        let extractor = PatchManifestExtractor::from_app_config(self.config);
        let save = self.config.feature_flags.is_save_patch_manifest;
        let mut patches = RegionResources::new();

        for (region, diffs) in decoded {
            let entries = if record {
                extractor.extract_root_one(region, diffs)
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

    async fn download_resources(&self, patches: &RegionResources) -> Result<()> {
        ResourcesFetcher::from_app_config(self.config)
            .fetch_and_save(patches)
            .await
    }

    fn merge_patches(&self, record: bool) -> Result<()> {
        if !self.config.feature_flags.is_merge_patch {
            logger::tips!("FEATURE: IS_MERGE_PATCH IS DISABLED.");
            return Ok(());
        }

        if record {
            logger::tips!("SKIP MERGE: record only dumps the manifest record.");
            return Ok(());
        }

        PatchMerger::from_app_config(self.config).apply_all()
    }

    fn save_version_file(&self, patches: &RegionResources, record: bool) -> Result<()> {
        let flags = &self.config.feature_flags;

        let patch_version_json: Value =
            if flags.is_extract_patch_version && !flags.is_use_resource_regex {
                let extractor = PatchVersionExtractor::from_app_config(self.config, patches);
                if record {
                    extractor.extract_mani(&self.config.file_name.update_root_mani_file)?
                } else {
                    extractor.extract(self.config.resource_registry.unpack_set())?
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
        let path = Path::new(&self.config.file_path.versions_output_dir)
            .join(&self.config.file_name.version_file);

        FileHandler::write_json(version, &path)?;
        log::info!("saved version json path= {}", path.display());

        Ok(())
    }
}

fn deep_merge(base: &mut Value, overlay: Value) {
    let Value::Object(overlay_map) = overlay else {
        return;
    };

    let Some(base_map) = base.as_object_mut() else {
        log::warn!("Skipped deep merge: base value is not a JSON object");
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
