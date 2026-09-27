use std::path::Path;
use std::path::PathBuf;

use regex::Regex;
use anyhow::Result;
use serde_json::Map;
use serde_json::Value;

use crate::config::AppConfig;
use crate::utils::ManiFileReader;
use crate::config::FileNameConfigs;
use crate::config::ExtractorConfig;
use crate::extractor::TResEntryIndexMap;
use crate::extractor::TManifestEntryIndexMap;


type TAllRegionPatches<'a> = &'a TManifestEntryIndexMap;


/// 补丁版本号提取器 自动遍历所有区域补丁清单返回结构化 JSON
pub struct PatchVersionExtractor<'a> {
    patch_tag_prefix: String,
    patch_tag_suffix: String,
    all_region_patches: TAllRegionPatches<'a>,
    #[allow(dead_code)]
    app_config: &'static AppConfig,
}

impl<'a> PatchVersionExtractor<'a> {
    pub fn new(all_region_patches: TAllRegionPatches<'a>) -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let filename_cfg: FileNameConfigs  = app_config.file_name.clone();

        Self { 
            patch_tag_prefix: filename_cfg.update_resource_patch_tag_prefix.clone(),
            patch_tag_suffix: filename_cfg.update_resource_patch_tag_suffix.clone(),
            all_region_patches,
            app_config,
        }
    }

    /// 对指定文件后缀的补丁在所有区域清单下批量查询提取
    pub fn extract<I, S>(&self, file_suffixes: I) -> Result<Value>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        logger::head!("EXTRACT PATCH VERSION");
        let mut result: Map<String, Value> = Map::new();
        let suffixes: Vec<String> = file_suffixes
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect();

        for (region, region_data) in self.all_region_patches {
            let mut suffix_map: Map<String, Value> = Map::new();
            for suffix_ref in &suffixes {
                let patch_version: String = self.match_patch_version(region, region_data, suffix_ref)?;
                suffix_map.insert(suffix_ref.clone(), serde_json::json!(patch_version));
            }
            result.insert(region.clone(), Value::Object(suffix_map));
        }

        Ok(Value::Object(result))
    }

    /// 提取主热更清单的版本
    pub fn extract_mani(&self, filename: &str) -> Result<Value>
    {
        logger::head!("EXTRACT PATCH VERSION");
        let mut result: Map<String, Value> = Map::new();
        for (region, region_data) in self.all_region_patches {
            let mut suffix_map: Map<String, Value> = Map::new();
            for (key, entry) in region_data { 
                if key == filename {
                    let patch_version: u64 = entry.version as u64;
                    suffix_map.insert("patch_ver".to_string(), serde_json::json!(&patch_version.to_string()));
                    log::debug!("[{:<2}]  patch_ver={}", region, patch_version);
                    break;
                }
            }            
            result.insert(region.clone(), Value::Object(suffix_map));
        }

        Ok(Value::Object(result))
    }

    /// 对单个区域的补丁清单执行补丁版本捕获
    fn match_patch_version(&self, region: &str, region_data: &TResEntryIndexMap, file_suffix: &str) -> Result<String> {
        let pattern_str: String = format!(
            r"^{}(\d+){}\.{}$", 
            self.patch_tag_prefix,
            self.patch_tag_suffix ,
            regex::escape(file_suffix),
        );

        let pattern: Regex = Regex::new(&pattern_str)
            .map_err(|e| anyhow::anyhow!("regex compile err for '{}': {}", file_suffix, e))?;

        struct PatchEntry {
            num: u64,
            version: u64,
        }

        let mut matching_versions: Vec<PatchEntry> = Vec::new();
        for (key, entry) in region_data {
            if let Some(caps) = pattern.captures(key) {
                let num: u64 = caps[1].parse().unwrap_or(0);
                matching_versions.push(PatchEntry {
                    num,
                    version: entry.version as u64,
                });
            }
        }

        if matching_versions.is_empty() {
            return Err(
                anyhow::anyhow!(
                    "no '{}.{}' formated patch found",
                    self.patch_tag_suffix,
                    file_suffix,
                )
            );
        }

        let mut highest: PatchEntry = matching_versions
            .into_iter()
            .max_by_key(|e| e.num)
            .unwrap();

        // key 优先级：同名 key 且 version 更高时覆盖
        if let Some(base_entry) = region_data.get(file_suffix) {
            let base_version: u64 = base_entry.version as u64;
            if base_version > highest.version {
                highest = PatchEntry {
                    num: 0,
                    version: base_version,
                };
            }
        }

        log::debug!(
            "[{:<2}]  {:<12}  num={:<4}  ver={}",
            region,
            file_suffix,
            highest.num,
            highest.version
        );

        Ok(format!("v{} (p{})", highest.version, highest.num))
    }
}


/// 游戏客户端版本提取器 自动遍历所有区域主热更清单补丁文件并提取 JSON
pub struct GameVersionExtractor {
    default_version_key: String,
    client_version_key: String,
    game_version_key: String,
    patch_version_key: String,
    patch_metadata_path: String,
    root_manifest_patch_filename: String,
    app_config: &'static AppConfig,
}

impl GameVersionExtractor { 
    pub fn new() -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let extractor_cfg: ExtractorConfig = app_config.extractor.clone();

        Self { 
            default_version_key:  extractor_cfg.default_version_key.clone(),
            client_version_key: extractor_cfg.client_version_key.clone(),
            game_version_key: extractor_cfg.game_version_key.clone(),
            patch_version_key: extractor_cfg.patch_version_key.clone(),
            patch_metadata_path: app_config.file_path.patch_output_dir.clone(),
            root_manifest_patch_filename: app_config.file_name.update_root_mani_file.clone(),
            app_config,
        }
    }

    /// 清单补丁 ss_win.mani 文件路径
    fn manifest_patch_path(&self, region: &str) -> PathBuf {
        Path::new(&self.patch_metadata_path)
            .join(region)
            .join(&self.root_manifest_patch_filename)
    }

    /// 提取所有区域的游戏版本和客户端版本
    pub fn extract_regions(&self) -> Result<Map<String, Value>> {
        logger::head!("EXTRACT CLIENT VERSION");
        let mut result: Map<String, Value> = Map::new();

        for region in self.app_config.available_regions() {
            let manifest_path: PathBuf = self.manifest_patch_path(region);
            let reader: ManiFileReader = ManiFileReader::new(manifest_path)?;
            
            let client_ver: &str = reader
                .get_config_value_by_key(&self.client_version_key)
                .unwrap_or(&self.default_version_key);

            let game_ver: &str = reader
                .get_config_value_by_key(&self.game_version_key)
                .unwrap_or(&self.default_version_key);

            let patch_ver: &str = reader
                .get_config_value_by_key(&self.patch_version_key)
                .unwrap_or(&self.default_version_key);

            log::debug!(
                "[{}] CLIENT_VER={}, GAME_VER={}, BIN_DIFF_PATCH_VER={}",
                region, client_ver, game_ver, patch_ver,
            );

            let version: String = format!("[c{}_g{}_p{}]", &client_ver, &game_ver, &patch_ver);
            let mut inner: Map<String, Value> = Map::new();

            inner.insert("game_ver".to_string(), Value::String(game_ver.to_string()));
            inner.insert("client_ver".to_string(), Value::String(client_ver.to_string()));
            inner.insert("patch_num".to_string(), Value::String(patch_ver.to_string()));
            inner.insert("version_str".to_string(), Value::String(version));
            result.insert(region.to_string(), Value::Object(inner));
        }

        Ok(result)
    }
}

impl Default for GameVersionExtractor {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use indexmap::IndexMap;
    use file_utils::FileHandler;
    use crate::config::ResourceEntry;

    #[test]
    fn test_func() -> Result<()> {
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();
        let mut all_region_patches: TManifestEntryIndexMap = IndexMap::new();

        for region in app_config.available_regions() {
            let patch_path: PathBuf = Path::new(&app_config.file_path.manifest_output_dir)
                .join(region)
                .join(&app_config.file_name.patch_manifest_file);

            if let Some(patches) = FileHandler::read_json::<TResEntryIndexMap>(&patch_path) {
                let filtered: IndexMap<String, ResourceEntry> = patches;
                all_region_patches.insert(region.to_string(), filtered);
            } else {
                logger::test!("Skip region {}: patch manifest not found or invalid", region);
            }
        }

        let version_extractor: PatchVersionExtractor<'_> = PatchVersionExtractor::new(&all_region_patches);
        let version_json: serde_json::Value =
            version_extractor.extract(app_config.resource_registry.unpack_set())?;
        logger::test!("Patch versions: {:#?}", version_json);

        let extractor: GameVersionExtractor = GameVersionExtractor::new();
        let result: Map<String, Value> = extractor.extract_regions()?;
        logger::test!("Game regions: {:#?}", result);

        Ok(())
    }
}