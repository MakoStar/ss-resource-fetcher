use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::config::resolve_platform_value;
use crate::model::Region;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorTaskConfig {
    /// 实际下载资源时使用的官方区域
    #[serde(rename = "SOURCE_REGION")]
    pub source_region: String,

    /// 路由配置
    #[serde(rename = "ROUTE")]
    pub route: UncensorRouteConfig,

    /// 清单所在服务器
    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, UncensorServerConfig>,

    /// 文件名
    #[serde(rename = "FILENAME")]
    pub filename: UncensorFileNameConfig,

    /// 输出目录
    #[serde(rename = "FILEPATH")]
    pub filepath: UncensorFilePathConfig,
}

impl Default for UncensorTaskConfig {
    fn default() -> Self {
        let mut servers = IndexMap::new();
        servers.insert(
            "CN".into(),
            UncensorServerConfig {
                url: "http://na.jvav.net.cn".into(),
            },
        );

        Self {
            source_region: "JP".into(),
            route: UncensorRouteConfig::default(),
            servers,
            filename: UncensorFileNameConfig::default(),
            filepath: UncensorFilePathConfig::default(),
        }
    }
}

impl UncensorTaskConfig {
    pub fn resolve(&mut self, platform: &str) {
        let template = Self::default();

        self.route.manifest =
            resolve_platform_value(&self.route.manifest, &template.route.manifest, platform);
        self.filename.manifest = resolve_platform_value(
            &self.filename.manifest,
            &template.filename.manifest,
            platform,
        );
        self.filepath.manifest = resolve_platform_value(
            &self.filepath.manifest,
            &template.filepath.manifest,
            platform,
        );
        self.filepath.metadata = resolve_platform_value(
            &self.filepath.metadata,
            &template.filepath.metadata,
            platform,
        );
        self.filepath.resource = resolve_platform_value(
            &self.filepath.resource,
            &template.filepath.resource,
            platform,
        );
        self.filepath.default =
            resolve_platform_value(&self.filepath.default, &template.filepath.default, platform);
    }

    pub fn server_url(&self, region: Option<&Region>) -> &str {
        region
            .and_then(|region| self.servers.get(region.as_str()))
            .or_else(|| self.servers.values().next())
            .map(|server| server.url.as_str())
            .unwrap_or("")
    }

    pub fn manifest_url(&self, region: Option<&Region>) -> String {
        format!(
            "{}/{}",
            self.server_url(region).trim_end_matches('/'),
            self.route.manifest.trim_start_matches('/'),
        )
    }

    pub fn resource_base_url(&self, region: Option<&Region>) -> String {
        let base = self.server_url(region).trim_end_matches('/');
        let route = self.route.manifest.trim_start_matches('/');

        match route.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => format!("{base}/{dir}"),
            _ => base.to_string(),
        }
    }

    pub fn manifest_file_name(&self) -> &str {
        self.route
            .manifest
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("ss_win.mani")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorRouteConfig {
    /// 清单在该服务器上的路径 含 {platform} 占位符
    #[serde(rename = "MANIFEST")]
    pub manifest: String,
}

impl Default for UncensorRouteConfig {
    fn default() -> Self {
        Self {
            manifest: "res/{platform}/ss_{platform}.mani".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorServerConfig {
    /// 服务器地址
    #[serde(rename = "URL")]
    pub url: String,
}

impl Default for UncensorServerConfig {
    fn default() -> Self {
        Self {
            url: "http://na.jvav.net.cn".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorFileNameConfig {
    /// 清单文件名 含 {platform} 占位符
    #[serde(rename = "MANIFEST")]
    pub manifest: String,
}

impl Default for UncensorFileNameConfig {
    fn default() -> Self {
        Self {
            manifest: "ss_{platform}.mani".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorFilePathConfig {
    /// 清单 json 输出目录
    #[serde(rename = "MANIFEST")]
    pub manifest: String,

    /// 下载的补丁资源目录
    #[serde(rename = "METADATA")]
    pub metadata: String,

    /// 最终资源输出目录
    #[serde(rename = "RESOURCE")]
    pub resource: String,

    /// 默认地址模式输出目录
    #[serde(rename = "DEFAULT")]
    pub default: String,
}

impl Default for UncensorFilePathConfig {
    fn default() -> Self {
        Self {
            manifest: "./output/Uncensor/Manifest".into(),
            metadata: "./output/Uncensor/Metadata".into(),
            resource: "./output/Uncensor/Resource".into(),
            default: "./output/Uncensor/Default".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UncensorServerConfig, UncensorTaskConfig};
    use crate::model::Region;

    #[test]
    fn resolve_substitutes_platform() {
        let mut config = UncensorTaskConfig::default();
        config.resolve("and");

        assert_eq!(config.route.manifest, "res/and/ss_and.mani");
        assert_eq!(config.filename.manifest, "ss_and.mani");
        assert_eq!(
            config.manifest_url(None),
            "http://na.jvav.net.cn/res/and/ss_and.mani"
        );
        assert_eq!(
            config.resource_base_url(None),
            "http://na.jvav.net.cn/res/and"
        );
    }

    #[test]
    fn manifest_url_follows_requested_region() {
        let mut config = UncensorTaskConfig::default();
        config.resolve("win");
        config.servers.insert(
            "TW".into(),
            UncensorServerConfig {
                url: "http://tw.example.com".into(),
            },
        );

        assert_eq!(
            config.manifest_url(Some(&Region::new("TW"))),
            "http://tw.example.com/res/win/ss_win.mani"
        );
        assert_eq!(
            config.manifest_url(None),
            "http://na.jvav.net.cn/res/win/ss_win.mani"
        );
    }

    #[test]
    fn manifest_url_falls_back_for_unknown_region() {
        let mut config = UncensorTaskConfig::default();
        config.resolve("win");

        assert_eq!(
            config.manifest_url(Some(&Region::new("TW"))),
            "http://na.jvav.net.cn/res/win/ss_win.mani"
        );
    }

    #[test]
    fn resolve_switches_platform_when_default_changed() {
        let mut config = UncensorTaskConfig::default();
        config.resolve("win");
        assert_eq!(config.route.manifest, "res/win/ss_win.mani");

        config.resolve("and");
        assert_eq!(config.route.manifest, "res/and/ss_and.mani");
    }

    #[test]
    fn resolve_keeps_custom_filepath_value() {
        let mut config = UncensorTaskConfig::default();
        config.filepath.metadata = "./my_own/metadata".into();
        config.resolve("and");

        assert_eq!(config.filepath.metadata, "./my_own/metadata");
    }

    #[test]
    fn resolve_substitutes_filepath_placeholder() {
        let mut config = UncensorTaskConfig::default();
        config.filepath.metadata = "./output/Uncensor/Metadata/{platform}".into();
        config.resolve("and");

        assert_eq!(config.filepath.metadata, "./output/Uncensor/Metadata/and");
    }
}
