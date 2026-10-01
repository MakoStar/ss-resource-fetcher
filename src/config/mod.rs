use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use network_manager::FetcherConfig;
use serde::{Deserialize, Serialize};

use crate::model::Region;

mod comments;
mod registry;
mod sections;

pub use registry::*;
pub use sections::*;

pub const CONFIG_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename = "DEFAULT_REGION")]
    pub default_region: String,

    #[serde(rename = "SERVER_ROUTE")]
    pub server_route: ServerRouteConfig,

    #[serde(rename = "FILE_PATH")]
    pub file_path: FilePathConfig,

    #[serde(rename = "FILE_NAME")]
    pub file_name: FileNameConfig,

    #[serde(rename = "EXTRACTOR")]
    pub extractor: ExtractorConfig,

    #[serde(rename = "REQUEST")]
    pub request: RequestConfig,

    #[serde(rename = "RESOURCE_REGISTRY")]
    pub resource_registry: ResourceRegistryConfig,

    #[serde(rename = "UNCENSOR")]
    pub uncensor: UncensorConfig,

    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, ServerConfig>,

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

        sections::STATIC_RESOURCE_SERVERS
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

        // comments::apply(&content)
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

fn apply_default_region_filter(cfg: &mut AppConfig) {
    if cfg.feature_flags.is_use_default_region {
        let default_region: String = cfg.default_region.clone();
        cfg.servers.retain(|region, _| region == &default_region);
        logger::tips!(
            "IS_USE_DEFAULT_REGION is enabled, only '{}' region is kept !!!",
            cfg.default_region
        );
    }
}

pub fn load_or_init_config<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    let path: &Path = path.as_ref();

    if !path.exists() {
        let mut cfg: AppConfig = AppConfig::default();
        cfg.save(path)?;
        logger::tips!("APPLICATION CONFIGURATION FILE NOT FOUND, GENERATED DEFAULT CONFIG.");
        apply_default_region_filter(&mut cfg);
        return Ok(cfg);
    }

    let content: String = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut cfg: AppConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}, please check syntax", path.display()))?;

    apply_default_region_filter(&mut cfg);
    cfg.validate()?;

    Ok(cfg)
}

pub static APP_CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| load_or_init_config(CONFIG_FILE_NAME).expect("failed to load config"));

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn generates_default_file_when_missing() {
        let temp_dir: std::path::PathBuf = std::env::temp_dir().join("config_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_path: std::path::PathBuf = temp_dir.join("config.toml");

        let _ = std::fs::remove_file(&config_path);
        let _: AppConfig = load_or_init_config(&config_path).unwrap();

        assert!(config_path.exists(), "Config file was not created!");

        let content: String = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.is_empty(), "Config file is empty!");

        logger::test!(
            "Config generated successfully at: {}",
            config_path.display()
        );

        log::info!("Content:\n{}", content);

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
