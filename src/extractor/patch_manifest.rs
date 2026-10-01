use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::config::{AppConfig, ResourceRegistryConfig};
use crate::error::{AppError, Result};
use crate::generated::FileDiff;
use crate::model::{Region, RegionFileDiffs, RegionResources, ResourceEntries, ResourceEntry};

pub struct PatchManifestExtractor {
    patch_tag_prefix: String,
    manifest_extension: String,
    main_manifest_filename: String,
    first_manifest_patch_name: String,
    use_resource_regex: bool,
    registry: ResourceRegistryConfig,
}

impl PatchManifestExtractor {
    pub fn from_app_config(app: &AppConfig) -> Self {
        Self {
            patch_tag_prefix: app.file_name.update_resource_patch_tag_prefix.clone(),
            manifest_extension: app.file_name.update_manifest_extension.clone(),
            main_manifest_filename: app.file_name.update_root_mani_file.clone(),
            first_manifest_patch_name: app.file_name.update_first_manifest_patch_name.clone(),
            use_resource_regex: app.feature_flags.is_use_resource_regex,
            registry: app.resource_registry.clone(),
        }
    }

    pub fn extract(&self, decoded: &RegionFileDiffs) -> Result<RegionResources> {
        let mut result = IndexMap::with_capacity(decoded.len());

        for (region, diffs) in decoded {
            logger::head!("EXTRACT PATCH MANIFEST {region}");

            let resources = Self::to_resource_map(diffs);
            let selected = self.select_downloadable(&resources);
            let filtered = self.filter_outdated_patches(region, selected)?;

            result.insert(region.clone(), filtered);
        }

        Ok(result)
    }

    pub fn extract_root(&self, decoded: &RegionFileDiffs) -> Result<RegionResources> {
        let mut result = IndexMap::with_capacity(decoded.len());

        for (region, diffs) in decoded {
            logger::head!("EXTRACT ROOT MANIFEST {region}");

            let resources = Self::to_resource_map(diffs);
            result.insert(region.clone(), self.select_root_manifest(&resources));
        }

        Ok(result)
    }

    fn to_resource_map(diffs: &[FileDiff]) -> ResourceEntries {
        diffs
            .iter()
            .map(|diff| (diff.file_name.clone(), ResourceEntry::from(diff)))
            .collect()
    }

    fn select_where(
        resources: &ResourceEntries,
        predicate: impl Fn(&ResourceEntry) -> bool,
    ) -> ResourceEntries {
        resources
            .iter()
            .filter(|(_, entry)| {
                if entry.file_name.is_empty() {
                    log::warn!("Skipping resource without file name: {entry:?}");
                    return false;
                }
                predicate(entry)
            })
            .map(|(name, entry)| (name.clone(), entry.clone()))
            .collect()
    }

    fn is_fetchable(&self, file_name: &str) -> bool {
        if self.use_resource_regex {
            return self.registry.is_base_resource(file_name)
                || self.registry.is_base_resource_suffix(file_name);
        }

        self.registry.is_fetchable(file_name)
    }

    fn is_manifest_patch(&self, key: &str) -> bool {
        key.starts_with(&self.patch_tag_prefix) && key.ends_with(&self.manifest_extension)
    }

    fn select_downloadable(&self, resources: &ResourceEntries) -> ResourceEntries {
        Self::select_where(resources, |entry| self.is_fetchable(&entry.file_name))
    }

    fn select_root_manifest(&self, resources: &ResourceEntries) -> ResourceEntries {
        let main_manifest = self.main_manifest_filename.as_str();
        Self::select_where(resources, |entry| entry.file_name == main_manifest)
    }

    fn filter_outdated_patches(
        &self,
        region: &Region,
        mut entries: ResourceEntries,
    ) -> Result<ResourceEntries> {
        let base_version = self.base_manifest_version(&entries)?;

        let regex_matched: Vec<String> = if self.use_resource_regex {
            entries
                .keys()
                .filter(|key| self.registry.is_base_resource(key) || self.is_manifest_patch(key))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let base_resources: HashSet<&str> = if self.use_resource_regex {
            regex_matched.iter().map(String::as_str).collect()
        } else {
            self.registry.fetch_names()
        };

        let suffix_index = Self::build_suffix_index(&entries, &base_resources);
        let outdated: Vec<String> = entries
            .iter()
            .filter(|&(key, entry)| {
                self.is_patch_obsolete(key, entry, &base_resources, base_version, &suffix_index)
            })
            .map(|(key, _)| key.clone())
            .collect();

        let before = entries.len();
        for key in &outdated {
            entries.swap_remove(key);
        }

        log::debug!(
            "[{region}] kept {} of {} entries, dropped {} outdated patches",
            entries.len(),
            before,
            outdated.len(),
        );

        Ok(entries)
    }

    fn base_manifest_version(&self, resources: &ResourceEntries) -> Result<u64> {
        let main_key = &self.main_manifest_filename;
        let first_patch_key = &self.first_manifest_patch_name;

        resources
            .get(first_patch_key)
            .or_else(|| resources.get(main_key))
            .map(|entry| entry.version)
            .ok_or_else(|| {
                AppError::PatchVersion(format!(
                    "cannot resolve base manifest version: neither '{first_patch_key}' nor '{main_key}' present"
                ))
            })
    }

    fn is_patch_obsolete(
        &self,
        res_key: &str,
        res_data: &ResourceEntry,
        base_resources: &HashSet<&str>,
        base_version: u64,
        suffix_index: &HashMap<&str, u64>,
    ) -> bool {
        if base_resources.contains(res_key) {
            return false;
        }
        if self.is_manifest_patch(res_key) {
            return base_version > res_data.version;
        }

        Self::base_version_for_patch(res_key, suffix_index)
            .is_some_and(|base_version| res_data.version < base_version)
    }

    fn base_version_for_patch(res_key: &str, suffix_index: &HashMap<&str, u64>) -> Option<u64> {
        let mut start = 0;

        while let Some(offset) = res_key[start..].find('.') {
            let dot = start + offset;
            let suffix = &res_key[dot + 1..];
            if let Some(&version) = suffix_index.get(suffix) {
                return Some(version);
            }
            start = dot + 1;
        }

        None
    }

    fn build_suffix_index<'a>(
        resources: &'a ResourceEntries,
        base_resources: &HashSet<&'a str>,
    ) -> HashMap<&'a str, u64> {
        base_resources
            .iter()
            .filter_map(|&name| resources.get(name).map(|entry| (name, entry.version)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extractor_with_regex(use_regex: bool) -> PatchManifestExtractor {
        let mut config = AppConfig::default();
        config.feature_flags.is_use_resource_regex = use_regex;

        PatchManifestExtractor::from_app_config(&config)
    }

    fn entry(file_name: &str, version: u64) -> ResourceEntry {
        ResourceEntry {
            file_name: file_name.to_string(),
            hash: String::new(),
            version,
            additional_path: String::new(),
        }
    }

    #[test]
    fn selects_downloadable_by_name_list() {
        let extractor = extractor_with_regex(false);
        let mut resources = ResourceEntries::new();
        resources.insert("lua.arcx".into(), entry("lua.arcx", 1));
        resources.insert("p_3_u.lua.arcx".into(), entry("p_3_u.lua.arcx", 3));
        resources.insert("ss_win.mani".into(), entry("ss_win.mani", 2));
        resources.insert("unrelated.txt".into(), entry("unrelated.txt", 1));
        resources.insert(String::new(), entry("", 1));

        let selected = extractor.select_downloadable(&resources);

        assert!(selected.contains_key("lua.arcx"));
        assert!(selected.contains_key("p_3_u.lua.arcx"));
        assert!(selected.contains_key("ss_win.mani"));
        assert!(!selected.contains_key("unrelated.txt"));

        assert!(!selected.contains_key(""));
    }

    #[test]
    fn selects_downloadable_by_regex() {
        let extractor = extractor_with_regex(true);
        let mut resources = ResourceEntries::new();
        resources.insert("ss_win.mani".into(), entry("ss_win.mani", 2));
        resources.insert("p_3_m.ss_win.mani".into(), entry("p_3_m.ss_win.mani", 3));
        resources.insert("icon-abc.unity3d".into(), entry("icon-abc.unity3d", 1));
        resources.insert("lua.arcx".into(), entry("lua.arcx", 1));

        let selected = extractor.select_downloadable(&resources);

        assert!(selected.contains_key("ss_win.mani"));
        assert!(selected.contains_key("p_3_m.ss_win.mani"));
        assert!(selected.contains_key("icon-abc.unity3d"));
        assert!(!selected.contains_key("lua.arcx"));
    }

    #[test]
    fn selects_only_root_manifest() {
        let extractor = extractor_with_regex(false);
        let mut resources = ResourceEntries::new();
        resources.insert("ss_win.mani".into(), entry("ss_win.mani", 2));
        resources.insert("lua.arcx".into(), entry("lua.arcx", 1));

        let selected = extractor.select_root_manifest(&resources);

        assert_eq!(selected.len(), 1);
        assert!(selected.contains_key("ss_win.mani"));
    }

    #[test]
    fn resolves_base_version_by_suffix() {
        let mut index: HashMap<&str, u64> = HashMap::new();
        index.insert("lua.arcx", 10);

        assert_eq!(
            PatchManifestExtractor::base_version_for_patch("p_3_u.lua.arcx", &index),
            Some(10)
        );

        assert_eq!(
            PatchManifestExtractor::base_version_for_patch("a.b.lua.arcx", &index),
            Some(10)
        );

        assert_eq!(
            PatchManifestExtractor::base_version_for_patch("unknown.arcx", &index),
            None
        );
    }

    #[test]
    fn detects_obsolete_patches() {
        let extractor = extractor_with_regex(false);
        let base_resources: HashSet<&str> = ["lua.arcx"].into_iter().collect();
        let mut suffix_index: HashMap<&str, u64> = HashMap::new();
        suffix_index.insert("lua.arcx", 10);

        assert!(!extractor.is_patch_obsolete(
            "lua.arcx",
            &entry("lua.arcx", 1),
            &base_resources,
            5,
            &suffix_index,
        ));

        assert!(extractor.is_patch_obsolete(
            "p_3_u.lua.arcx",
            &entry("p_3_u.lua.arcx", 3),
            &base_resources,
            5,
            &suffix_index,
        ));

        assert!(!extractor.is_patch_obsolete(
            "p_12_u.lua.arcx",
            &entry("p_12_u.lua.arcx", 12),
            &base_resources,
            5,
            &suffix_index,
        ));

        assert!(extractor.is_patch_obsolete(
            "p_1_m.ss_win.mani",
            &entry("p_1_m.ss_win.mani", 1),
            &base_resources,
            5,
            &suffix_index,
        ));
        
        assert!(!extractor.is_patch_obsolete(
            "p_7_m.ss_win.mani",
            &entry("p_7_m.ss_win.mani", 7),
            &base_resources,
            5,
            &suffix_index,
        ));
    }
}
