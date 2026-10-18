use std::collections::HashMap;

use crate::config::{AppConfig, ClientRegistryConfig, DELTA_PATCH_TAG, NEW_FILE_PATCH_TAG};
use crate::error::{AppError, Result};
use crate::generated::FileDiff;
use crate::model::{PatchRef, Region, ResourceEntries, ResourceEntry, apply_order};

pub struct PatchManifestExtractor {
    /// 补丁标签前缀
    patch_tag_prefix: String,
    /// 清单扩展名
    manifest_extension: String,
    /// 主清单文件名
    main_manifest_filename: String,
    /// 首个清单补丁名
    first_manifest_patch_name: String,
    /// 是否启用正则过滤
    use_resource_regex: bool,
    /// 是否过滤过时补丁
    filter_outdated: bool,
    /// 资源注册表
    registry: ClientRegistryConfig,
}

impl PatchManifestExtractor {
    pub fn from_app_config(app: &AppConfig) -> Self {
        Self {
            patch_tag_prefix: app.updatetag.patch_prefix.clone(),
            manifest_extension: app.extension.mani.clone(),
            main_manifest_filename: app.task.client.filename.root_mani.clone(),
            first_manifest_patch_name: app.task.client.filename.first_mani_patch.clone(),
            use_resource_regex: app.task.client.flags.op.use_resource_regex,
            filter_outdated: app.task.client.flags.op.filter_outdated_patch,
            registry: app.task.client.registry.clone(),
        }
    }

    pub fn with_filter_outdated(mut self, filter: Option<bool>) -> Self {
        if let Some(enabled) = filter {
            self.filter_outdated = enabled;
        }

        self
    }

    pub fn extract_one(&self, region: &Region, diffs: &[FileDiff]) -> Result<ResourceEntries> {
        let resources = Self::to_resource_map(diffs);
        let mut selected = self.select_downloadable(&resources);
        self.drop_unowned_patches(region, &mut selected, &resources);

        if !self.filter_outdated {
            let selected = self.sort_download_order(&selected);
            log::info!(
                "[{region}] outdated patch filter off, kept {} entries",
                selected.len()
            );
            return Ok(selected);
        }

        let filtered = self.filter_outdated_patches(region, selected)?;
        Ok(self.sort_download_order(&filtered))
    }

    pub fn extract_root_one(&self, diffs: &[FileDiff]) -> ResourceEntries {
        let resources = Self::to_resource_map(diffs);
        self.select_root_manifest(&resources)
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
                || self.registry.is_base_resource_suffix(file_name)
                || self.registry.is_patch_resource(file_name);
        }

        self.registry.is_fetchable(file_name)
    }

    fn is_manifest_patch(&self, key: &str) -> bool {
        key.starts_with(&self.patch_tag_prefix) && key.ends_with(&self.manifest_extension)
    }

    fn select_downloadable(&self, resources: &ResourceEntries) -> ResourceEntries {
        Self::select_where(resources, |entry| self.is_fetchable(&entry.file_name))
    }

    fn drop_unowned_patches(
        &self,
        region: &Region,
        selected: &mut ResourceEntries,
        resources: &ResourceEntries,
    ) {
        let mut dropped = Vec::new();
        let mut orphan_delta = 0;

        for key in selected.keys() {
            let is_resource_patch = matches!(
                self.registry.patch_tag(key),
                Some(NEW_FILE_PATCH_TAG) | Some(DELTA_PATCH_TAG)
            );

            let Some(base) = self.registry.patch_base_name(key) else {
                continue;
            };

            if !is_resource_patch || selected.contains_key(base) {
                continue;
            }

            let has_base_row = resources.contains_key(base);

            if self.registry.is_new_file_patch(key) {
                if !has_base_row {
                    continue;
                }
            } else if !has_base_row {
                orphan_delta += 1;
            }

            dropped.push(key.clone());
        }

        if dropped.is_empty() {
            return;
        }

        for key in &dropped {
            selected.swap_remove(key);
        }

        log::debug!(
            "[{region}] dropped {} patch(es), base out of scope, {orphan_delta} without base entry",
            dropped.len()
        );
    }

    fn sort_download_order(&self, entries: &ResourceEntries) -> ResourceEntries {
        let mut items: Vec<(&String, &ResourceEntry)> = entries.iter().collect();

        items.sort_by_key(|(key, _)| self.registry.patch_sequence(key));

        items
            .into_iter()
            .map(|(key, entry)| (key.to_string(), entry.clone()))
            .collect()
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
        let manifest_version = self.base_manifest_version(&entries)?;
        let outdated = self.outdated_keys(&entries, manifest_version);

        let before = entries.len();
        for key in &outdated {
            entries.swap_remove(key);
        }

        log::info!(
            "[{region}] kept {} of {} entries, dropped {} outdated patches",
            entries.len(),
            before,
            outdated.len(),
        );

        Ok(entries)
    }

    fn outdated_keys(&self, entries: &ResourceEntries, manifest_version: u64) -> Vec<String> {
        let mut outdated: Vec<_> = entries
            .iter()
            .filter(|(key, entry)| self.is_manifest_patch(key) && manifest_version > entry.version)
            .map(|(key, _)| key.clone())
            .collect();

        for (base, patches) in self.patch_groups(entries) {
            let base_version = entries.get(base).map(|entry| entry.version);
            let kept = apply_order(base_version, &patches, |name| {
                self.registry.is_new_file_patch(name)
            });

            outdated.extend(
                patches
                    .iter()
                    .filter(|patch| !kept.contains(&patch.name))
                    .map(|patch| patch.name.to_string()),
            );

            let replaced = kept
                .first()
                .is_some_and(|name| self.registry.is_new_file_patch(name));

            if replaced && base_version.is_some() {
                outdated.push(base.to_string());
            }
        }

        outdated
    }

    fn patch_groups<'a>(
        &self,
        entries: &'a ResourceEntries,
    ) -> HashMap<&'a str, Vec<PatchRef<'a>>> {
        let mut groups: HashMap<&str, Vec<_>> = HashMap::new();

        for (key, entry) in entries {
            let is_resource_patch = matches!(
                self.registry.patch_tag(key),
                Some(NEW_FILE_PATCH_TAG) | Some(DELTA_PATCH_TAG)
            );

            let Some(base) = self.registry.patch_base_name(key) else {
                continue;
            };

            if is_resource_patch {
                groups.entry(base).or_default().push(PatchRef::new(
                    &self.registry,
                    key,
                    entry.version,
                ));
            }
        }

        groups
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extractor_with_regex(use_regex: bool) -> PatchManifestExtractor {
        extractor_with(use_regex, true)
    }

    fn extractor_with(use_regex: bool, filter_outdated: bool) -> PatchManifestExtractor {
        let mut config = AppConfig::default();
        config.task.client.flags.op.use_resource_regex = use_regex;
        config.task.client.flags.op.filter_outdated_patch = filter_outdated;

        PatchManifestExtractor::from_app_config(&config)
    }

    fn extractor_with_tags(tags: &[&str]) -> PatchManifestExtractor {
        let mut config = AppConfig::default();
        config.task.client.flags.op.use_resource_regex = true;

        let mut registry = ClientRegistryConfig::default();
        registry.res_patterns = vec![
            r"^ss_win(_add)?\.mani$".into(),
            r"^icon-[0-9A-Za-z]+\.unity3d$".into(),
            r"^char_2d_[0-9]+\.unity3d$".into(),
        ];
        registry.patch_prefix = "p_".into();
        registry.patch_tags = tags.iter().map(|tag| tag.to_string()).collect();
        config.task.client.registry = registry;

        PatchManifestExtractor::from_app_config(&config)
    }

    fn diff(file_name: &str, version: i64) -> FileDiff {
        FileDiff {
            file_name: file_name.to_string(),
            hash: String::new(),
            version,
            additional_path: String::new(),
        }
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
    fn download_order_groups_patches_by_tier() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("TW");
        let diffs = vec![
            diff("icon-a.unity3d", 91),
            diff("ss_win.mani", 137),
            diff("p_1_u.icon-a.unity3d", 101),
            diff("p_1_m.ss_win.mani", 119),
            diff("p_2_u.icon-a.unity3d", 102),
            diff("p_2_m.ss_win.mani", 120),
            diff("p_3_u.icon-a.unity3d", 103),
            diff("p_12_n.char_2d_13901.unity3d", 147),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();
        let order: Vec<_> = selected.keys().map(String::as_str).collect();

        assert_eq!(
            order,
            vec![
                "icon-a.unity3d",
                "ss_win.mani",
                "p_1_u.icon-a.unity3d",
                "p_1_m.ss_win.mani",
                "p_2_u.icon-a.unity3d",
                "p_2_m.ss_win.mani",
                "p_3_u.icon-a.unity3d",
                "p_12_n.char_2d_13901.unity3d",
            ]
        );
    }

    #[test]
    fn regex_mode_drops_obsolete_manifest_patches() {
        let extractor = extractor_with(true, true);
        let region = Region::new("TW");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("ss_win_add.mani", 137),
            diff("p_11_m.ss_win.mani", 137),
            diff("p_12_m.ss_win.mani", 114),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("ss_win.mani"));
        assert!(selected.contains_key("ss_win_add.mani"));
        assert!(selected.contains_key("p_11_m.ss_win.mani"));
        assert!(!selected.contains_key("p_12_m.ss_win.mani"));
    }

    #[test]
    fn outdated_override_beats_config() {
        let diffs = vec![diff("ss_win.mani", 137), diff("p_12_m.ss_win.mani", 114)];
        let region = Region::new("TW");

        let forced_on = extractor_with(true, false).with_filter_outdated(Some(true));
        let kept = forced_on.extract_one(&region, &diffs).unwrap();
        assert!(!kept.contains_key("p_12_m.ss_win.mani"));

        let forced_off = extractor_with(true, true).with_filter_outdated(Some(false));
        let kept = forced_off.extract_one(&region, &diffs).unwrap();
        assert!(kept.contains_key("p_12_m.ss_win.mani"));

        let inherit = extractor_with(true, false).with_filter_outdated(None);
        let kept = inherit.extract_one(&region, &diffs).unwrap();
        assert!(kept.contains_key("p_12_m.ss_win.mani"));
    }

    #[test]
    fn outdated_filter_off_keeps_every_patch() {
        let extractor = extractor_with(true, false);
        let region = Region::new("TW");
        let diffs = vec![diff("ss_win.mani", 137), diff("p_12_m.ss_win.mani", 114)];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert_eq!(selected.len(), 2);
        assert!(selected.contains_key("p_12_m.ss_win.mani"));
    }

    #[test]
    fn selected_patch_tags_join_download_scope() {
        let mut resources = ResourceEntries::new();
        resources.insert(
            "p_11_n.char_2d_13902.unity3d".into(),
            entry("p_11_n.char_2d_13902.unity3d", 137),
        );

        let mut config = AppConfig::default();
        config.task.client.flags.op.use_resource_regex = true;
        config.task.client.registry.patch_tags = vec!["u".into()];
        let extractor = PatchManifestExtractor::from_app_config(&config);
        assert!(
            !extractor
                .select_downloadable(&resources)
                .contains_key("p_11_n.char_2d_13902.unity3d")
        );

        config.task.client.registry.patch_tags = vec!["n".into()];
        let extractor = PatchManifestExtractor::from_app_config(&config);
        let selected = extractor.select_downloadable(&resources);

        assert!(selected.contains_key("p_11_n.char_2d_13902.unity3d"));
    }

    #[test]
    fn new_file_patch_drops_replaced_base_file() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("TW");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("char_2d_13901.unity3d", 91),
            diff("p_12_n.char_2d_13901.unity3d", 147),
            diff("icon-abc.unity3d", 10),
            diff("p_3_u.icon-abc.unity3d", 12),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("p_12_n.char_2d_13901.unity3d"));
        assert!(!selected.contains_key("char_2d_13901.unity3d"));
        assert!(selected.contains_key("p_3_u.icon-abc.unity3d"));
        assert!(selected.contains_key("icon-abc.unity3d"));
    }

    #[test]
    fn patches_with_out_of_scope_base_are_dropped() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("CN");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("icon-a.unity3d", 91),
            diff("p_3_u.icon-a.unity3d", 103),
            diff("Monster_14011.bnk", 91),
            diff("p_17_u.Monster_14011.bnk", 120),
            diff("p_12_n.char_2d_13901.unity3d", 147),
            diff("p_9_u.char_2d_13902.unity3d", 148),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("p_3_u.icon-a.unity3d"));
        assert!(!selected.contains_key("p_17_u.Monster_14011.bnk"));
        assert!(selected.contains_key("p_12_n.char_2d_13901.unity3d"));
        assert!(!selected.contains_key("p_9_u.char_2d_13902.unity3d"));
    }

    #[test]
    fn name_list_mode_drops_unowned_patches() {
        let extractor = extractor_with_regex(false);
        let region = Region::new("CN");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("lua.arcx", 91),
            diff("p_3_u.lua.arcx", 103),
            diff("Monster_14011.bnk", 91),
            diff("p_17_u.Monster_14011.bnk", 120),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("p_3_u.lua.arcx"));
        assert!(!selected.contains_key("p_17_u.Monster_14011.bnk"));
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
    fn new_file_patch_drops_older_delta() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("TW");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("char_2d_13901.unity3d", 91),
            diff("p_12_n.char_2d_13901.unity3d", 147),
            diff("p_1_u.char_2d_13901.unity3d", 146),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("p_12_n.char_2d_13901.unity3d"));
        assert!(!selected.contains_key("p_1_u.char_2d_13901.unity3d"));
        assert!(!selected.contains_key("char_2d_13901.unity3d"));
    }

    #[test]
    fn newer_delta_merges_on_top_of_new_file() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("TW");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("char_2d_13901.unity3d", 91),
            diff("p_12_n.char_2d_13901.unity3d", 147),
            diff("p_13_u.char_2d_13901.unity3d", 148),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("p_12_n.char_2d_13901.unity3d"));
        assert!(selected.contains_key("p_13_u.char_2d_13901.unity3d"));
        assert!(!selected.contains_key("char_2d_13901.unity3d"));
    }

    #[test]
    fn deltas_without_new_file_keep_the_base_file() {
        let extractor = extractor_with_tags(&["u", "n"]);
        let region = Region::new("TW");
        let diffs = vec![
            diff("ss_win.mani", 137),
            diff("icon-abc.unity3d", 10),
            diff("p_1_u.icon-abc.unity3d", 120),
            diff("p_2_u.icon-abc.unity3d", 130),
        ];

        let selected = extractor.extract_one(&region, &diffs).unwrap();

        assert!(selected.contains_key("icon-abc.unity3d"));
        assert!(selected.contains_key("p_1_u.icon-abc.unity3d"));
        assert!(selected.contains_key("p_2_u.icon-abc.unity3d"));
    }
}
