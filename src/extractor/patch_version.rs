use regex::Regex;
use serde_json::{Map, Value};

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::model::{Region, RegionResources, ResourceEntries};

struct PatchCandidate {
    /// 补丁序号
    num: u64,
    /// 补丁版本
    version: u64,
}

pub struct PatchVersionExtractor<'a> {
    /// 补丁标签前缀
    patch_tag_prefix: String,
    /// 补丁标签后缀
    patch_tag_suffix: String,
    /// 各区域补丁资源
    all_region_patches: &'a RegionResources,
}

impl<'a> PatchVersionExtractor<'a> {
    pub fn from_app_config(app: &AppConfig, all_region_patches: &'a RegionResources) -> Self {
        Self {
            patch_tag_prefix: app.file_name.update_resource_patch_tag_prefix.clone(),
            patch_tag_suffix: app.file_name.update_resource_patch_tag_suffix.clone(),
            all_region_patches,
        }
    }

    pub fn extract<I, S>(&self, file_suffixes: I) -> Result<Value>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        logger::head!("EXTRACT PATCH VERSION");

        let suffixes: Vec<String> = file_suffixes
            .into_iter()
            .map(|suffix| suffix.as_ref().to_string())
            .collect();

        let compiled: Vec<(&str, Regex)> = suffixes
            .iter()
            .map(|suffix| Ok((suffix.as_str(), self.compile_pattern(suffix)?)))
            .collect::<Result<Vec<_>>>()?;

        let mut result = Map::with_capacity(self.all_region_patches.len());

        for (region, region_data) in self.all_region_patches {
            let mut suffix_map = Map::with_capacity(compiled.len());

            for (suffix, pattern) in &compiled {
                let version = self.match_patch_version(region, region_data, suffix, pattern)?;
                suffix_map.insert((*suffix).to_string(), Value::String(version));
            }

            result.insert(region.to_string(), Value::Object(suffix_map));
        }

        Ok(Value::Object(result))
    }

    pub fn extract_mani(&self, filename: &str) -> Result<Value> {
        logger::head!("EXTRACT PATCH VERSION");

        let mut result = Map::with_capacity(self.all_region_patches.len());

        for (region, region_data) in self.all_region_patches {
            let mut suffix_map = Map::new();

            if let Some(entry) = region_data.get(filename) {
                let patch_version = entry.version;
                suffix_map.insert(
                    "patch_ver".to_string(),
                    Value::String(patch_version.to_string()),
                );
                log::info!("[{region:<2}]  patch_ver={patch_version}");
            }

            result.insert(region.to_string(), Value::Object(suffix_map));
        }

        Ok(Value::Object(result))
    }

    fn compile_pattern(&self, file_suffix: &str) -> Result<Regex> {
        let pattern_str = format!(
            r"^{}(\d+){}\.{}$",
            self.patch_tag_prefix,
            self.patch_tag_suffix,
            regex::escape(file_suffix),
        );

        Regex::new(&pattern_str)
            .map_err(|err| AppError::message(format!("invalid regex for '{file_suffix}': {err}")))
    }

    fn match_patch_version(
        &self,
        region: &Region,
        region_data: &ResourceEntries,
        file_suffix: &str,
        pattern: &Regex,
    ) -> Result<String> {
        let mut highest = region_data
            .iter()
            .filter_map(|(key, entry)| {
                let caps = pattern.captures(key)?;
                Some(PatchCandidate {
                    num: caps[1].parse().unwrap_or(0),
                    version: entry.version,
                })
            })
            .max_by_key(|candidate| candidate.num)
            .ok_or_else(|| {
                AppError::message(format!(
                    "no patch of form '{}.{}' found",
                    self.patch_tag_suffix, file_suffix,
                ))
            })?;

        if let Some(base_entry) = region_data.get(file_suffix)
            && base_entry.version > highest.version
        {
            highest = PatchCandidate {
                num: 0,
                version: base_entry.version,
            };
        }

        log::debug!(
            "[{region:<2}]  {file_suffix:<12}  num={:<4}  ver={}",
            highest.num,
            highest.version
        );

        Ok(format!("v{} (p{})", highest.version, highest.num))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ResourceEntry;

    fn entry(file_name: &str, version: u64) -> ResourceEntry {
        ResourceEntry {
            file_name: file_name.to_string(),
            hash: String::new(),
            version,
            additional_path: String::new(),
        }
    }

    fn sample_patches() -> RegionResources {
        let mut patches = RegionResources::new();
        let mut entries = ResourceEntries::new();

        entries.insert("lua.arcx".into(), entry("lua.arcx", 5));
        entries.insert("p_1_u.lua.arcx".into(), entry("p_1_u.lua.arcx", 7));
        entries.insert("p_3_u.lua.arcx".into(), entry("p_3_u.lua.arcx", 9));
        entries.insert("p_12_u.lua.arcx".into(), entry("p_12_u.lua.arcx", 11));

        patches.insert(Region::new("CN"), entries);
        patches
    }

    fn extractor_with(patches: &RegionResources) -> PatchVersionExtractor<'_> {
        PatchVersionExtractor::from_app_config(&AppConfig::default(), patches)
    }

    #[test]
    fn picks_highest_sequence_patch() {
        let patches = sample_patches();
        let extractor = extractor_with(&patches);
        let pattern = extractor.compile_pattern("lua.arcx").unwrap();

        let resources = patches.get(&Region::new("CN")).unwrap();
        let version = extractor
            .match_patch_version(&Region::new("CN"), resources, "lua.arcx", &pattern)
            .unwrap();

        assert_eq!(version, "v11 (p12)");
    }

    #[test]
    fn base_resource_overrides_when_newer() {
        let mut patches = RegionResources::new();
        let mut entries = ResourceEntries::new();

        entries.insert("lua.arcx".into(), entry("lua.arcx", 99));
        entries.insert("p_1_u.lua.arcx".into(), entry("p_1_u.lua.arcx", 3));
        patches.insert(Region::new("CN"), entries);

        let extractor = extractor_with(&patches);
        let pattern = extractor.compile_pattern("lua.arcx").unwrap();

        let resources = patches.get(&Region::new("CN")).unwrap();
        let version = extractor
            .match_patch_version(&Region::new("CN"), resources, "lua.arcx", &pattern)
            .unwrap();

        assert_eq!(version, "v99 (p0)");
    }

    #[test]
    fn reports_missing_patch() {
        let patches = sample_patches();
        let extractor = extractor_with(&patches);
        let pattern = extractor.compile_pattern("data.arcx").unwrap();

        let resources = patches.get(&Region::new("CN")).unwrap();
        let err = extractor
            .match_patch_version(&Region::new("CN"), resources, "data.arcx", &pattern)
            .unwrap_err();

        assert!(err.to_string().contains("data.arcx"));
    }

    #[test]
    fn extracts_mani_patch_version() {
        let patches = sample_patches();
        let extractor = extractor_with(&patches);

        let value = extractor.extract_mani("lua.arcx").unwrap();
        let cn = value.get("CN").and_then(|v| v.get("patch_ver")).unwrap();

        assert_eq!(cn, &Value::String("5".to_string()));
    }
}
