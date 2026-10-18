use std::path::Path;

use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::client::{ClientServerConfig, STATIC_CLIENT_SERVERS};
use super::{
    CommentLang, ExtensionConfig, GlobalConfig, NetworkConfig, PLATFORMS, TaskConfig,
    UpdateTagConfig, config_comments_lang,
};
use crate::model::Region;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 全局配置
    #[serde(rename = "GLOBAL", default)]
    pub global: GlobalConfig,

    /// 网络配置
    #[serde(rename = "NETWORK", default)]
    pub network: NetworkConfig,

    /// 文件扩展名配置
    #[serde(rename = "FILE_EXTENSION", default)]
    pub extension: ExtensionConfig,

    /// 补丁标签配置
    #[serde(rename = "PATCH_UPDATE_TAG", default)]
    pub updatetag: UpdateTagConfig,

    /// 任务配置
    #[serde(rename = "TASK", default)]
    pub task: TaskConfig,
}

impl AppConfig {
    #[inline(always)]
    pub fn get() -> Result<&'static AppConfig> {
        crate::config::APP_CONFIG
            .as_ref()
            .map_err(|err| anyhow!("{err}"))
    }

    pub fn available_regions(&self) -> impl Iterator<Item = &str> {
        self.task.client.servers.keys().map(|s| s.as_str())
    }

    pub fn regions(&self) -> Vec<Region> {
        self.available_regions().map(Region::new).collect()
    }

    pub fn server_urls(&self) -> IndexMap<Region, String> {
        self.task
            .client
            .servers
            .iter()
            .map(|(name, server)| (Region::new(name), server.url.clone()))
            .collect()
    }

    pub fn server_config(&self, region: &Region) -> Option<ClientServerConfig> {
        if let Some(config) = self.task.client.servers.get(region.as_str()) {
            return Some(config.clone());
        }

        STATIC_CLIENT_SERVERS
            .get(region.as_str())
            .map(ClientServerConfig::from)
    }

    pub fn build_fetcher_config(&self) -> network_manager::FetcherConfig {
        self.network.build_fetcher_config()
    }

    pub fn apply_platform(&mut self) {
        let platform = self.global.default_platform.clone();
        let prefix = self.updatetag.patch_prefix.clone();
        let tags = self.updatetag.patch_tags.clone();

        self.task.client.resolve(&platform);
        self.task.client.link_updatetag(&prefix, &tags);
        self.task.uncensor.resolve(&platform);
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path: &Path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create dir {}", parent.display()))?;
        }
        let content = self.to_toml(config_comments_lang());
        std::fs::write(path, content)
            .with_context(|| format!("failed to write {}", path.display()))?;

        Ok(())
    }

    fn to_toml(&self, lang: Option<CommentLang>) -> String {
        let content = toml::to_string_pretty(self).expect("failed to serialize AppConfig");

        if let Some(lang) = lang {
            return super::comments::apply(&content, lang);
        }

        content
    }

    pub fn validate(&self) -> Result<()> {
        if !self
            .task
            .client
            .servers
            .contains_key(&self.global.default_region)
        {
            anyhow::bail!(
                "DEFAULT_REGION '{}' not found in client servers. Available: {}",
                self.global.default_region,
                self.task
                    .client
                    .servers
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        if !PLATFORMS.contains(&self.global.default_platform.as_str()) {
            anyhow::bail!(
                "DEFAULT_PLATFORM '{}' is not supported. Available: {}",
                self.global.default_platform,
                PLATFORMS.join(", ")
            );
        }

        let dirs: [(&str, &str); 9] = [
            (
                "CLIENT.FILEPATH.ROOT_OUTPUT",
                &self.task.client.filepath.root_output,
            ),
            (
                "CLIENT.FILEPATH.MANIFEST",
                &self.task.client.filepath.manifest,
            ),
            (
                "CLIENT.FILEPATH.VERSIONS",
                &self.task.client.filepath.versions,
            ),
            ("CLIENT.FILEPATH.PATCH", &self.task.client.filepath.patch),
            ("CLIENT.FILEPATH.UNPACK", &self.task.client.filepath.unpack),
            (
                "LAUNCHER.FILEPATH.OUTPUT",
                &self.task.launcher.filepath.output,
            ),
            (
                "UNCENSOR.FILEPATH.MANIFEST",
                &self.task.uncensor.filepath.manifest,
            ),
            (
                "UNCENSOR.FILEPATH.METADATA",
                &self.task.uncensor.filepath.metadata,
            ),
            (
                "UNCENSOR.FILEPATH.RESOURCE",
                &self.task.uncensor.filepath.resource,
            ),
        ];

        for (key, dir) in dirs {
            if dir.is_empty() {
                anyhow::bail!("Directory path '{key}' cannot be empty");
            }
        }

        let names: [(&str, &str); 11] = [
            (
                "CLIENT.FILENAME.RAW_MANIFEST",
                &self.task.client.filename.raw_manifest,
            ),
            (
                "CLIENT.FILENAME.DECRYPTED_MANIFEST",
                &self.task.client.filename.decrypted_manifest,
            ),
            (
                "CLIENT.FILENAME.DECODED_MANIFEST",
                &self.task.client.filename.decoded_manifest,
            ),
            (
                "CLIENT.FILENAME.PATCH_MANIFEST",
                &self.task.client.filename.patch_manifest,
            ),
            (
                "CLIENT.FILENAME.VERSION",
                &self.task.client.filename.version,
            ),
            (
                "CLIENT.FILENAME.ROOT_MANI",
                &self.task.client.filename.root_mani,
            ),
            (
                "CLIENT.FILENAME.FIRST_MANI_PATCH",
                &self.task.client.filename.first_mani_patch,
            ),
            (
                "CLIENT.FILENAME.ADD_MANI",
                &self.task.client.filename.add_mani,
            ),
            (
                "LAUNCHER.FILENAME.MANIFEST",
                &self.task.launcher.filename.manifest,
            ),
            (
                "UNCENSOR.FILENAME.MANIFEST",
                &self.task.uncensor.filename.manifest,
            ),
            (
                "UNCENSOR.FILEPATH.DEFAULT",
                &self.task.uncensor.filepath.default,
            ),
        ];

        for (key, name) in names {
            if name.is_empty() {
                anyhow::bail!("File name '{key}' cannot be empty");
            }
        }

        if self.network.timeout_secs == 0 {
            anyhow::bail!("NETWORK.TIMEOUT_SECS must be greater than 0");
        }

        if self.task.client.flags.op.use_resource_regex {
            for pattern in &self.task.client.registry.res_patterns {
                regex::Regex::new(pattern)
                    .with_context(|| format!("Invalid resource regex pattern: '{pattern}'"))?;
            }
        }

        self.task.launcher.patterns()?;

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut config = Self {
            global: GlobalConfig::default(),
            network: NetworkConfig::default(),
            extension: ExtensionConfig::default(),
            updatetag: UpdateTagConfig::default(),
            task: TaskConfig::default(),
        };
        config.apply_platform();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let mut config = AppConfig::default();
        config.apply_platform();
        let toml_str = config.to_toml(Some(crate::config::CommentLang::En));
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.global.default_region, config.global.default_region);
        assert_eq!(
            parsed.task.client.servers.len(),
            config.task.client.servers.len()
        );
    }
}
