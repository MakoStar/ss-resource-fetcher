use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy)]
pub struct StaticLauncherServer {
    /// 启动器上报的游戏标识
    pub tag: &'static str,
    /// 签名盐值
    pub salt: &'static str,
    /// 配置接口域名
    pub api_url: &'static str,
    /// 资源包域名
    pub pkg_url: &'static str,
}

pub(crate) static STATIC_LAUNCHER_SERVERS: phf::OrderedMap<&'static str, StaticLauncherServer> = phf::phf_ordered_map! {
    "CN" => StaticLauncherServer {
        tag: "StellaSora_CN",
        salt: "872550AD59A235662C5B7D5F88CEBE4B",
        api_url: "https://launcher-api.yostar.net",
        pkg_url: "https://game-launcher-ss-cn.yostar.net"
    },
    "EN" => StaticLauncherServer {
        tag: "StellaSora_EN",
        salt: "DE7108E9B2842FD460F4777702727869",
        api_url: "https://api-launcher-en.yo-star.com",
        pkg_url: "https://launcher-pkg-ss-en.yo-star.com"
    },
    "JP" => StaticLauncherServer {
        tag: "StellaSora_JP",
        salt: "DE7108E9B2842FD460F4777702727869",
        api_url: "https://api-launcher-jp.yo-star.com",
        pkg_url: "https://launcher-pkg-ss-jp.yo-star.com"
    },
    "KR" => StaticLauncherServer {
        tag: "StellaSora_KR",
        salt: "DE7108E9B2842FD460F4777702727869",
        api_url: "https://api-launcher-kr.yo-star.com",
        pkg_url: "https://launcher-pkg-ss-kr.yo-star.com"
    },
    "TW" => StaticLauncherServer {
        tag: "StellaSora_TW",
        salt: "DE7108E9B2842FD460F4777702727869",
        api_url: "https://api-launcher-tw.stargazer-games.com",
        pkg_url: "https://launcher-pkg-ss-hk.stargazer-games.com"
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherServerConfig {
    /// 启动器上报的游戏标识
    #[serde(rename = "TAG")]
    pub tag: String,

    /// 请求签名盐值
    #[serde(rename = "SALT")]
    pub salt: String,

    /// 配置接口域名
    #[serde(rename = "API_URL")]
    pub api_url: String,

    /// 资源包域名
    #[serde(rename = "PKG_URL")]
    pub pkg_url: String,
}

impl From<&StaticLauncherServer> for LauncherServerConfig {
    fn from(s: &StaticLauncherServer) -> Self {
        Self {
            tag: s.tag.into(),
            salt: s.salt.into(),
            api_url: s.api_url.into(),
            pkg_url: s.pkg_url.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    /// 启动器版本号
    #[serde(rename = "VERSION")]
    pub version: String,

    /// 配置接口路径
    #[serde(rename = "CONFIG_ROUTE")]
    pub config_route: String,

    /// 清单直链查询参数模板
    #[serde(rename = "CONFIG_PARAMS")]
    pub config_params: String,

    /// 返回体里的版本字段名
    #[serde(rename = "VERSION_KEY")]
    pub version_key: String,

    /// 返回体里的路径字段名
    #[serde(rename = "PATH_KEY")]
    pub path_key: String,

    /// 是否保存 launcher 清单
    #[serde(rename = "IS_SAVE_MANIFEST")]
    pub is_save_manifest: bool,

    /// 资源筛选正则
    #[serde(rename = "PATTERNS")]
    pub patterns: Vec<String>,

    /// 指定下载的文件列表
    #[serde(rename = "FILES")]
    pub files: Vec<String>,

    /// 各区域启动器端点
    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, LauncherServerConfig>,

    /// 编译后的正则 不参与序列化
    #[serde(skip)]
    compiled: Arc<OnceLock<Vec<Regex>>>,
}

impl LauncherConfig {
    pub fn patterns(&self) -> Result<&[Regex]> {
        if let Some(cached) = self.compiled.get() {
            return Ok(cached.as_slice());
        }

        let compiled: Vec<Regex> = compile_patterns(&self.patterns)?;
        Ok(self.compiled.get_or_init(|| compiled).as_slice())
    }

    pub fn config_url(&self, server: &LauncherServerConfig) -> String {
        format!(
            "{}{}",
            server.api_url.trim_end_matches('/'),
            self.config_route
        )
    }

    pub fn manifest_link_url(
        &self,
        server: &LauncherServerConfig,
        version: &str,
        file_path: &str,
    ) -> String {
        let params: String = self
            .config_params
            .replace("{ver}", version)
            .replace("{path}", file_path);

        format!("{}{}", self.config_url(server), params)
    }

    pub fn resource_url(server: &LauncherServerConfig, source: &str, path: &str) -> String {
        let source: &str = source.trim_matches('/');
        let path: &str = path.trim_start_matches('/');
        let base: &str = server.pkg_url.trim_end_matches('/');

        if source.is_empty() {
            format!("{base}/{path}")
        } else {
            format!("{base}/{source}/{path}")
        }
    }

    pub fn region_names(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(|name| name.as_str())
    }

    pub fn has_region(&self, region: &str) -> bool {
        self.servers.contains_key(region) || STATIC_LAUNCHER_SERVERS.contains_key(region)
    }

    pub fn server(&self, region: &str) -> Option<LauncherServerConfig> {
        if let Some(server) = self.servers.get(region) {
            return Some(server.clone());
        }

        STATIC_LAUNCHER_SERVERS
            .get(region)
            .map(LauncherServerConfig::from)
    }

    pub fn builtin_region_names(&self) -> impl Iterator<Item = &str> {
        STATIC_LAUNCHER_SERVERS.keys().copied()
    }
}

impl Default for LauncherConfig {
    fn default() -> Self {
        let mut servers: IndexMap<String, LauncherServerConfig> = IndexMap::new();
        for (&region, server) in STATIC_LAUNCHER_SERVERS.entries() {
            servers.insert(region.to_string(), LauncherServerConfig::from(server));
        }

        Self {
            version: "1.6.0".into(),
            config_route: "/api/launcher/game/config".into(),
            config_params: "/json?version={ver}&file_path={path}".into(),
            version_key: "game_latest_version".into(),
            path_key: "game_latest_file_path".into(),
            is_save_manifest: true,
            patterns: vec![r"^data\.unity3d$".into()],
            files: Vec::new(),
            servers,
            compiled: Arc::new(OnceLock::new()),
        }
    }
}

pub fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    let mut compiled: Vec<Regex> = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        let regex: Regex = Regex::new(pattern).map_err(|err| {
            AppError::message(format!("invalid launcher regex '{pattern}': {err}"))
        })?;
        compiled.push(regex);
    }

    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::{LauncherConfig, LauncherServerConfig, compile_patterns};
    use crate::error::AppError;

    fn launcher_server() -> LauncherServerConfig {
        LauncherServerConfig {
            tag: "StellaSora_KR".into(),
            salt: "salt".into(),
            api_url: "https://api.example.com/".into(),
            pkg_url: "https://pkg.example.com".into(),
        }
    }

    #[test]
    fn builds_launcher_config_urls() {
        let config = LauncherConfig::default();
        let server = launcher_server();

        assert_eq!(
            config.config_url(&server),
            "https://api.example.com/api/launcher/game/config"
        );
        assert_eq!(
            config.manifest_link_url(&server, "1.2.3", "prod/a/game.zip"),
            "https://api.example.com/api/launcher/game/config/json?version=1.2.3&file_path=prod/a/game.zip"
        );
    }

    #[test]
    fn builds_launcher_resource_urls() {
        let server = launcher_server();

        assert_eq!(
            LauncherConfig::resource_url(&server, "/game-1.2.3", "/a/b/data.unity3d"),
            "https://pkg.example.com/game-1.2.3/a/b/data.unity3d"
        );
        assert_eq!(
            LauncherConfig::resource_url(&server, "", "a/data.unity3d"),
            "https://pkg.example.com/a/data.unity3d"
        );
    }

    #[test]
    fn launcher_patterns_are_compiled_once() {
        let config = LauncherConfig::default();
        let first = config.patterns().expect("patterns should compile");
        let second = config.patterns().expect("patterns should compile");

        assert!(std::ptr::eq(first, second));
        assert!(first.iter().any(|regex| regex.is_match("data.unity3d")));
        assert!(!first.iter().any(|regex| regex.is_match("other.unity3d")));
    }

    #[test]
    fn invalid_launcher_pattern_is_reported() {
        let err: AppError = compile_patterns(&["(".into()]).unwrap_err();
        assert!(err.to_string().contains("invalid launcher regex"));
    }

    #[test]
    fn launcher_defaults_cover_all_builtin_regions() {
        let config = LauncherConfig::default();

        assert_eq!(config.servers.len(), 5);
        assert_eq!(config.servers["KR"].tag, "StellaSora_KR");
        assert!(config.servers.contains_key("CN"));
    }

    #[test]
    fn region_names_follow_server_order() {
        let config = LauncherConfig::default();

        assert_eq!(
            config.region_names().collect::<Vec<_>>(),
            vec!["CN", "EN", "JP", "KR", "TW"]
        );
    }

    #[test]
    fn server_falls_back_to_builtin_endpoints() {
        let mut config = LauncherConfig::default();
        config.servers.retain(|region, _| region == "EN");

        assert!(config.has_region("CN"));
        assert_eq!(
            config.server("CN").expect("built-in CN").tag,
            "StellaSora_CN"
        );
        assert!(!config.has_region("ZZ"));
        assert!(config.server("ZZ").is_none());
    }
}
