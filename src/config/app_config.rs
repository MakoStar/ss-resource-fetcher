use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use network_manager::FetcherConfig;
use serde::{Deserialize, Serialize};

use super::sections::STATIC_RESOURCE_SERVERS;
use crate::config::{APP_CONFIG, ServerConfig};
use crate::config::{ExtractorConfig, FeatureFlags, FileNameConfig, ResourceRegistryConfig};
use crate::config::{FilePathConfig, RequestConfig, ServerRouteConfig, UncensorConfig};
use crate::model::Region;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 默认区域
    #[serde(rename = "DEFAULT_REGION")]
    pub default_region: String,

    /// 服务器路由
    #[serde(rename = "SERVER_ROUTE")]
    pub server_route: ServerRouteConfig,

    /// 输出目录
    #[serde(rename = "FILE_PATH")]
    pub file_path: FilePathConfig,

    /// 文件名
    #[serde(rename = "FILE_NAME")]
    pub file_name: FileNameConfig,

    /// 版本提取
    #[serde(rename = "EXTRACTOR")]
    pub extractor: ExtractorConfig,

    /// 网络请求
    #[serde(rename = "REQUEST")]
    pub request: RequestConfig,

    /// 资源注册表
    #[serde(rename = "RESOURCE_REGISTRY")]
    pub resource_registry: ResourceRegistryConfig,

    /// 反和谐配置
    #[serde(rename = "UNCENSOR")]
    pub uncensor: UncensorConfig,

    /// 各区域服务器
    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, ServerConfig>,

    /// 功能开关
    #[serde(rename = "FEATURE_FLAGS")]
    pub feature_flags: FeatureFlags,
}

impl AppConfig {
    #[inline(always)]
    pub fn get() -> &'static AppConfig {
        &APP_CONFIG
    }

    pub fn available_regions(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(|s| s.as_str())
    }

    pub fn regions(&self) -> Vec<Region> {
        self.available_regions().map(Region::new).collect()
    }

    pub fn server_urls(&self) -> IndexMap<Region, String> {
        self.servers
            .iter()
            .map(|(name, server)| (Region::new(name), server.url.clone()))
            .collect()
    }

    pub fn server_config(&self, region: &Region) -> Option<ServerConfig> {
        if let Some(config) = self.servers.get(region.as_str()) {
            return Some(config.clone());
        }

        super::sections::STATIC_RESOURCE_SERVERS
            .get(region.as_str())
            .map(ServerConfig::from)
    }

    pub fn build_fetcher_config(&self) -> FetcherConfig {
        self.request.build_fetcher_config()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path: &Path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create dir {}", parent.display()))?;
        }
        let content: String = self.to_toml_with_comments();
        std::fs::write(path, content)
            .with_context(|| format!("failed to write {}", path.display()))?;

        Ok(())
    }

    fn to_toml_with_comments(&self) -> String {
        let content: String = toml::to_string_pretty(self).expect("failed to serialize AppConfig");

        content
    }

    pub fn validate(&self) -> Result<()> {
        if !self.servers.contains_key(&self.default_region) {
            anyhow::bail!(
                "default_region '{}' not found in servers list. Available: {}",
                self.default_region,
                self.servers.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        let dirs: [(&str, &str); 7] = [
            ("ROOT_OUTPUT_DIR", &self.file_path.root_output_dir),
            ("MANIFEST_OUTPUT_DIR", &self.file_path.manifest_output_dir),
            ("VERSIONS_OUTPUT_DIR", &self.file_path.versions_output_dir),
            ("PATCH_OUTPUT_DIR", &self.file_path.patch_output_dir),
            ("UNPACK_OUTPUT_DIR", &self.file_path.unpack_output_dir),
            ("UNCENSOR_OUTPUT_DIR", &self.file_path.uncensor_output_dir),
            (
                "UNCENSOR_DEFAULT_OUTPUT_DIR",
                &self.file_path.uncensor_default_output_dir,
            ),
        ];

        for (key, dir) in dirs {
            if dir.is_empty() {
                anyhow::bail!("Directory path '{key}' cannot be empty");
            }
        }

        let names: [(&str, &str); 5] = [
            ("MANIFEST_RAW_FILE", &self.file_name.manifest_raw_file),
            (
                "MANIFEST_DECRYPT_FILE",
                &self.file_name.manifest_decrypt_file,
            ),
            (
                "MANIFEST_DECODED_FILE",
                &self.file_name.manifest_decoded_file,
            ),
            ("PATCH_MANIFEST_FILE", &self.file_name.patch_manifest_file),
            ("VERSION_FILE", &self.file_name.version_file),
        ];

        for (key, name) in names {
            if name.is_empty() {
                anyhow::bail!("File name '{key}' cannot be empty");
            }
        }

        if self.request.timeout_secs == 0 {
            anyhow::bail!("TIMEOUT_SECS must be greater than 0");
        }

        if self.feature_flags.is_use_resource_regex {
            for pattern in &self.resource_registry.base_resource_patterns {
                regex::Regex::new(pattern)
                    .with_context(|| format!("Invalid resource regex pattern: '{pattern}'"))?;
            }
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut servers: IndexMap<String, ServerConfig> = IndexMap::new();
        for (&region, cfg) in STATIC_RESOURCE_SERVERS.entries() {
            servers.insert(region.to_string(), ServerConfig::from(cfg));
        }

        Self {
            default_region: "CN".into(),
            server_route: ServerRouteConfig::default(),
            file_path: FilePathConfig::default(),
            file_name: FileNameConfig::default(),
            extractor: ExtractorConfig::default(),
            request: RequestConfig::default(),
            resource_registry: ResourceRegistryConfig::default(),
            uncensor: UncensorConfig::default(),
            servers,
            feature_flags: FeatureFlags::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_config_is_valid() {
        let config: AppConfig = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let config: AppConfig = AppConfig::default();
        let toml_str: String = config.to_toml_with_comments();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.default_region, config.default_region);
        assert_eq!(parsed.servers.len(), config.servers.len());
    }
}
