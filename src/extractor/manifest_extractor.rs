use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use file_utils::FileHandler;
use indexmap::IndexMap;

use crate::config::FileNameConfigs;
use crate::config::FilePathConfigs;
use crate::config::ResourceEntry;
use crate::decoder::TDecodedManifestIndexMap;

use crate::config::AppConfig;
use crate::config::ResourceRegistryConfig;

pub type TResEntryIndexMap = IndexMap<String, ResourceEntry>;
pub type TNeedFetchResourceMap = IndexMap<String, ResourceEntry>;
pub type TManifestEntryIndexMap = IndexMap<String, TResEntryIndexMap>;

pub struct HotfixPatchManifestExtractor {
    patch_tag_prefix: String,
    manifest_extension: String,
    main_manifest_filename: String,
    first_manifest_patch_name: String,
    patch_manifest_output_path: String,
    patch_manifest_filename: String,
    app_config: &'static AppConfig,
}

impl HotfixPatchManifestExtractor {
    pub fn new() -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let file_name: FileNameConfigs = app_config.file_name.clone();
        let file_path: FilePathConfigs = app_config.file_path.clone();

        Self {
            patch_tag_prefix: file_name.update_resource_patch_tag_prefix,
            manifest_extension: file_name.update_manifest_extension,
            main_manifest_filename: file_name.update_root_mani_file,
            first_manifest_patch_name: file_name.update_first_manifest_patch_name,
            patch_manifest_output_path: file_path.manifest_output_dir,
            patch_manifest_filename: file_name.patch_manifest_file,
            app_config,
        }
    }

    /// 补丁清单文件保存的路径
    fn patch_manifest_path(&self, region: &str) -> PathBuf {
        Path::new(&self.patch_manifest_output_path)
            .join(&region)
            .join(&self.patch_manifest_filename)
    }

    /// 判断是否是需要获取的资源补丁
    fn is_need_fetch(&self, file_name: &str) -> bool {
        let registry: &ResourceRegistryConfig = &self.app_config.resource_registry;
        if self.app_config.feature_flags.is_use_resource_regex {                  
            return registry.is_base_resource(file_name)
                || registry.is_base_resource_suffix(file_name);
        }

        registry.need_fetch_set().contains(file_name)
            || registry
                .fetchable_extensions()
                .iter()
                .any(|dotted| file_name.ends_with(dotted.as_str()))
    }

    /// 检查是否是资源清单的补丁
    #[inline]
    fn is_manifest_patch(&self, key: &str) -> bool {
        key.starts_with(&self.patch_tag_prefix) && key.ends_with(&self.manifest_extension)
    }

    /// 获取清单补丁的版本 基于首个热更的补丁 如果没有首个补丁则用主清单补丁的版本
    fn manifest_patch_version(&self, resources: &TResEntryIndexMap) -> Result<u64> {
        let base_main_key: &String = &self.main_manifest_filename;
        let first_update_key: &String = &self.first_manifest_patch_name;
        resources
            .get(first_update_key)
            .or_else(|| resources.get(base_main_key))
            .map(|e| e.version)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "update patch ss_win.mani base version error: \
                     neither '{first_update_key}' nor '{base_main_key}' found"
                )
            })
    }

    /// 判断是否是过时补丁
    fn is_patch_obsolete(
        &self,
        res_key: &str,
        res_data: &ResourceEntry,
        base_res: &HashSet<&str>,
        base_ver: u64,
        suffix_index: &HashMap<&str, u64>,
    ) -> bool {
        if base_res.contains(res_key) {
            return false;
        }
        if self.is_manifest_patch(res_key) {
            return base_ver > res_data.version;
        }
        Self::base_version_for_patch(res_key, suffix_index)
            .is_some_and(|base_ver| res_data.version < base_ver)
    }

    /// 根据补丁资源键查找其对应的基础版本号
    fn base_version_for_patch(res_key: &str, suffix_index: &HashMap<&str, u64>) -> Option<u64> {
        let mut start: usize = 0;
        while let Some(pos) = res_key[start..].find('.') {
            let abs_pos: usize = start + pos;
            let suffix: &str = &res_key[abs_pos + 1..];
            if let Some(&ver) = suffix_index.get(suffix) {
                return Some(ver);
            }
            start = abs_pos + 1;
        }
        None
    }

    /// 构建基础资源的后缀版本号索引
    fn build_suffix_index<'a>(
        &self,
        res: &'a TResEntryIndexMap,
        base_res: &'a HashSet<&'a str>,
    ) -> HashMap<&'a str, u64> {
        base_res
            .iter()
            .filter_map(|&name| res.get(name).map(|entry| (name, entry.version)))
            .collect()
    }

    /// 收集所有已过时的补丁资源键
    fn collect_outdated_keys(
        &self,
        resources: &TResEntryIndexMap,
        base_res: &HashSet<&str>,
        base_ver: u64,
        suffix_index: &HashMap<&str, u64>,
    ) -> Vec<String> {
        resources
            .iter()
            .filter_map(|(res_key, res_data)| {
                self.is_patch_obsolete(res_key, res_data, base_res, base_ver, suffix_index)
                    .then(|| res_key.clone())
            })
            .collect()
    }

    /// 过滤过时的补丁包
    fn filter_outdated_patches(
        &self,
        region: &str,
        resources: &mut TResEntryIndexMap,
        base_resources: &HashSet<&str>,
    ) -> Result<()> {
        let ss_win_ver: u64 = self.manifest_patch_version(resources)?;
        let suffix_index: HashMap<&str, u64> = self.build_suffix_index(resources, base_resources);
        let outdated_keys: Vec<String> =
            self.collect_outdated_keys(resources, base_resources, ss_win_ver, &suffix_index);

        let before_len: usize = resources.len();

        for key in &outdated_keys {
            resources.swap_remove(key);
        }

        log::debug!(
            "[{region}] remaining({}) - removing {} outdated patches out of {} total",
            resources.len(),
            outdated_keys.len(),
            before_len,
        );

        Ok(())
    }

    /// 从 manifest 中提取需要拉取的补丁清单
    fn extract_need_fetch_map(&self, manifest_data: &TResEntryIndexMap) -> TNeedFetchResourceMap {
        let mut need_fetch_map = IndexMap::new();
        for (_, entry) in manifest_data {
            if entry.file_name.is_empty() {
                log::warn!("Resource: not found file name -> {:?}", entry);
                continue;
            }
            if !self.is_need_fetch(&entry.file_name) {
                continue;
            }
            need_fetch_map.insert(
                entry.file_name.clone(),
                ResourceEntry {
                    file_name: entry.file_name.clone(),
                    hash: entry.hash.clone(),
                    version: entry.version,
                    additional_path: entry.additional_path.clone(),
                },
            );
        }
        need_fetch_map
    }

    /// 从 manifest 中提取需要主补丁清单的配置数据
    fn extract_root_manifest(&self, manifest_data: &TResEntryIndexMap) -> TNeedFetchResourceMap {
        let mut root_manifest_map: IndexMap<_, _> = IndexMap::new();
        for (_, entry) in manifest_data {
            if entry.file_name.is_empty() {
                log::warn!("Resource: not found file name -> {:?}", entry);
                continue;
            }

            if entry.file_name != self.app_config.file_name.update_root_mani_file {
                continue;
            }
            root_manifest_map.insert(
                entry.file_name.clone(),
                ResourceEntry {
                    file_name: entry.file_name.clone(),
                    hash: entry.hash.clone(),
                    version: entry.version,
                    additional_path: entry.additional_path.clone(),
                },
            );
        }
        root_manifest_map
    }

    /// 过滤掉过期的补丁条目返回干净的清单
    // fn filter_patch_manifest(
    //     &self,
    //     region: &str,
    //     data_map: &TNeedFetchResourceMap,
    // ) -> Result<TResEntryIndexMap> {
    //     let mut filtered_map: TResEntryIndexMap = data_map.clone();
    //     let base_resources: HashSet<&str> = self.app_config.resource_registry.need_fetch_set();
    //     self.filter_outdated_patches(region, &mut filtered_map, &base_resources)?;

    //     Ok(filtered_map)
    // }
    fn filter_patch_manifest(
        &self,
        region: &str,
        data_map: &TNeedFetchResourceMap,
    ) -> Result<TResEntryIndexMap> {
        let mut filtered_map: TResEntryIndexMap = data_map.clone();
        let registry: &ResourceRegistryConfig = &self.app_config.resource_registry;

        // 1. 先把正则命中的 key clone 成 owned，生命周期独立于 filtered_map
        let regex_matched: Vec<String> = if self.app_config.feature_flags.is_use_resource_regex {
            filtered_map
                .keys()
                .filter(|k| {
                    registry.is_base_resource(k)
                        || self.is_manifest_patch(k)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
  
        let base_resources: HashSet<&str> = if self.app_config.feature_flags.is_use_resource_regex {
            regex_matched.iter().map(|s| s.as_str()).collect()
        } else {
            registry.need_fetch_set()
        };

        self.filter_outdated_patches(region, &mut filtered_map, &base_resources)?;
        Ok(filtered_map)
    }

    /// 将补丁清单写入 JSON 文件
    fn write_patch_manifest(
        &self,
        region: &str,
        data_map: &TResEntryIndexMap,
        _: &str,
    ) -> Result<()> {
        let path: PathBuf = self.patch_manifest_path(region);
        FileHandler::write_json(data_map, &path)?;
        log::info!("[{region}] saved path= {}", path.display());

        Ok(())
    }

    /// 提取并保存补丁清单
    pub fn extract_and_save_patch_manifest(
        &self,
        region: &str,
        manifest_data: &TResEntryIndexMap,
    ) -> Result<TResEntryIndexMap> {
        let need_fetch_manifests: TResEntryIndexMap = self.extract_need_fetch_map(manifest_data);
        let filtered_manifest_map: TResEntryIndexMap =
            self.filter_patch_manifest(region, &need_fetch_manifests)?;
        if self.app_config.feature_flags.is_save_patch_manifest {
            self.write_patch_manifest(region, &filtered_manifest_map, "filtered")?;
        }
        Ok(filtered_manifest_map)
    }

    /// 提取并保存主补丁清单
    pub fn extract_and_save_root_manifest(
        &self,
        region: &str,
        manifest_data: &TResEntryIndexMap,
    ) -> Result<TResEntryIndexMap> {
        let root_manifests: TResEntryIndexMap = self.extract_root_manifest(manifest_data);
        if self.app_config.feature_flags.is_save_patch_manifest {
            self.write_patch_manifest(region, &root_manifests, "root-manifest")?;
        }
        Ok(root_manifests)
    }

    /// 从解码数据开始提取, 返回每个 region 对应的过滤后补丁清单
    pub fn extract_from_data(
        &self,
        data: &TDecodedManifestIndexMap,
    ) -> Result<TManifestEntryIndexMap> {
        let mut result: TManifestEntryIndexMap = IndexMap::new();
        for (region, file_diffs) in data {
            logger::head!("EXTRACT PATCH MANIFEST {}", region);
            let resource_map: TResEntryIndexMap = file_diffs
                .iter()
                .map(|fd| (fd.file_name.clone(), ResourceEntry::from(fd)))
                .collect();

            let filtered: IndexMap<String, ResourceEntry> =
                self.extract_and_save_patch_manifest(region, &resource_map)?;
            result.insert(region.clone(), filtered);
        }

        Ok(result)
    }

    /// 用于提取主更新清单的 hash 配置表
    pub fn extract_root_manifest_from_data(
        &self,
        data: &TDecodedManifestIndexMap,
    ) -> Result<TManifestEntryIndexMap> {
        let mut result: TManifestEntryIndexMap = IndexMap::new();
        for (region, file_diffs) in data {
            logger::head!("EXTRACT ROOT MANIFEST {}", region);
            let resource_map: TResEntryIndexMap = file_diffs
                .iter()
                .map(|fd| (fd.file_name.clone(), ResourceEntry::from(fd)))
                .collect();

            let root_manifest: IndexMap<String, ResourceEntry> =
                self.extract_and_save_root_manifest(region, &resource_map)?;
            result.insert(region.clone(), root_manifest);
        }

        Ok(result)
    }
}

impl Default for HotfixPatchManifestExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::FileDiff;
    use indexmap::IndexMap;

    #[tokio::test]
    async fn test_func() -> Result<()> {
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();
        let mut all_maps: IndexMap<String, Vec<FileDiff>> = IndexMap::new();

        for region in app_config.available_regions() {
            let test_path: PathBuf = Path::new("./test")
                .join(region)
                .join(app_config.file_name.manifest_decoded_file.clone());

            if let Some(flat_map) = FileHandler::read_json::<IndexMap<String, FileDiff>>(&test_path)
            {
                for (_, v) in flat_map {
                    all_maps
                        .entry(region.to_string())
                        .or_insert_with(Vec::new)
                        .push(v);
                }
            } else {
                log::warn!(
                    "[{}] failed to read manifest from {}",
                    region,
                    test_path.display()
                );
            }
        }
        let extractor: HotfixPatchManifestExtractor = HotfixPatchManifestExtractor::new();
        let result: IndexMap<String, IndexMap<String, ResourceEntry>> =
            extractor.extract_from_data(&all_maps)?;

        for (key, value) in result {
            let save_path: PathBuf = Path::new("./test")
                .join(key)
                .join(app_config.file_name.patch_manifest_file.clone());

            FileHandler::write_json(&value, save_path)?;
        }

        Ok(())
    }
}
