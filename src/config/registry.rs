use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct CompiledRegistry {
    /// 完整匹配正则
    full_patterns: Vec<Regex>,
    /// 后缀匹配正则
    suffix_patterns: Vec<Regex>,
    /// 待抓取文件名集合
    fetch_names: HashSet<String>,
    /// 待抓取文件的后缀列表
    fetch_extensions: Vec<String>,
    /// 待解包文件名集合
    unpack_names: HashSet<String>,
}

impl CompiledRegistry {
    fn build(registry: &ResourceRegistryConfig) -> Self {
        let mut full_patterns = Vec::with_capacity(registry.base_resource_patterns.len());
        let mut suffix_patterns = Vec::with_capacity(registry.base_resource_patterns.len());

        for pattern in &registry.base_resource_patterns {
            match Regex::new(pattern) {
                Ok(regex) => full_patterns.push(regex),
                Err(err) => log::warn!("Invalid resource regex '{pattern}': {err}"),
            }

            let suffix_pattern = pattern.strip_prefix('^').unwrap_or(pattern);
            match Regex::new(suffix_pattern) {
                Ok(regex) => suffix_patterns.push(regex),
                Err(err) => log::warn!("Invalid suffix regex '{suffix_pattern}': {err}"),
            }
        }

        let fetch_names: HashSet<String> = registry.need_fetch_files.iter().cloned().collect();

        let fetch_extensions = registry
            .need_fetch_files
            .iter()
            .map(|name| format!(".{name}"))
            .collect();

        let unpack_names: HashSet<String> = registry.unpack_files.iter().cloned().collect();

        Self {
            full_patterns,
            suffix_patterns,
            fetch_names,
            fetch_extensions,
            unpack_names,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceRegistryConfig {
    /// 需要获取的文件列表
    #[serde(rename = "NEED_FETCH_FILES")]
    pub need_fetch_files: Vec<String>,

    /// 需要解包的文件列表
    #[serde(rename = "UNPACK_FILES")]
    pub unpack_files: Vec<String>,

    /// 基础资源正则模式
    #[serde(rename = "BASE_RESOURCE_PATTERNS")]
    pub base_resource_patterns: Vec<String>,

    /// 编译后的注册表 不参与序列化
    #[serde(skip)]
    compiled: Arc<OnceLock<CompiledRegistry>>,
}

impl ResourceRegistryConfig {
    fn compiled(&self) -> &CompiledRegistry {
        self.compiled.get_or_init(|| CompiledRegistry::build(self))
    }

    pub fn fetch_names(&self) -> HashSet<&str> {
        self.compiled()
            .fetch_names
            .iter()
            .map(String::as_str)
            .collect()
    }

    pub fn unpack_set(&self) -> &HashSet<String> {
        &self.compiled().unpack_names
    }

    pub fn is_base_resource(&self, name: &str) -> bool {
        self.compiled()
            .full_patterns
            .iter()
            .any(|regex| regex.is_match(name))
    }

    pub fn is_base_resource_suffix(&self, name: &str) -> bool {
        self.compiled()
            .suffix_patterns
            .iter()
            .any(|regex| regex.is_match(name))
    }

    pub fn is_fetchable(&self, file_name: &str) -> bool {
        let compiled = self.compiled();

        compiled.fetch_names.contains(file_name)
            || compiled
                .fetch_extensions
                .iter()
                .any(|extension| file_name.ends_with(extension.as_str()))
    }
}

impl Default for ResourceRegistryConfig {
    fn default() -> Self {
        Self {
            need_fetch_files: vec![
                "lua.arcx".into(),
                "data.arcx".into(),
                "lua.json".into(),
                "data.json".into(),
                "ss_win.mani".into(),
                "ss_win_add.mani".into(),
            ],
            unpack_files: vec!["lua.arcx".into(), "data.arcx".into()],
            base_resource_patterns: vec![
                r"^ss_win(_add)?\.mani$".into(),
                r"^icon-[0-9A-Za-z]+\.unity3d$".into(),
                r"^image-[0-9A-Za-z]+\.unity3d$".into(),
            ],
            compiled: Arc::new(OnceLock::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_registry_matching() {
        let registry: ResourceRegistryConfig = ResourceRegistryConfig::default();

        assert!(registry.is_base_resource("ss_win.mani"));
        assert!(registry.is_base_resource("ss_win_add.mani"));
        assert!(registry.is_base_resource("icon-abc123.unity3d"));
        assert!(!registry.is_base_resource("p_3_m.ss_win.mani"));

        assert!(registry.is_base_resource_suffix("p_3_m.ss_win.mani"));
        assert!(!registry.is_base_resource_suffix("lua.arcx"));

        assert!(registry.is_fetchable("lua.arcx"));
        assert!(registry.is_fetchable("p_12_u.lua.arcx"));
        assert!(registry.is_fetchable("ss_win.mani"));
        assert!(!registry.is_fetchable("unrelated.txt"));
    }

    #[test]
    fn compiled_registry_is_shared_between_clones() {
        let registry: ResourceRegistryConfig = ResourceRegistryConfig::default();
        let clone: ResourceRegistryConfig = registry.clone();

        assert!(std::ptr::eq(registry.compiled(), clone.compiled()));
    }
}
