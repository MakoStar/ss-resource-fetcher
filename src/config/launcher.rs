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
    pub api: &'static str,
    /// 资源包域名
    pub pkg: &'static str,
}

pub static STATIC_LAUNCHER_SERVERS: phf::OrderedMap<&'static str, StaticLauncherServer> = phf::phf_ordered_map! {
    "CN" => StaticLauncherServer {
        tag: "StellaSora_CN",
        salt: "872550AD59A235662C5B7D5F88CEBE4B",
        api: "https://launcher-api.yostar.net",
        pkg: "https://game-launcher-ss-cn.yostar.net"
    },
    "EN" => StaticLauncherServer {
        tag: "StellaSora_EN",
        salt: "DE7108E9B2842FD460F4777702727869",
        api: "https://api-launcher-en.yo-star.com",
        pkg: "https://launcher-pkg-ss-en.yo-star.com"
    },
    "JP" => StaticLauncherServer {
        tag: "StellaSora_JP",
        salt: "DE7108E9B2842FD460F4777702727869",
        api: "https://api-launcher-jp.yo-star.com",
        pkg: "https://launcher-pkg-ss-jp.yo-star.com"
    },
    "KR" => StaticLauncherServer {
        tag: "StellaSora_KR",
        salt: "DE7108E9B2842FD460F4777702727869",
        api: "https://api-launcher-kr.yo-star.com",
        pkg: "https://launcher-pkg-ss-kr.yo-star.com"
    },
    "TW" => StaticLauncherServer {
        tag: "StellaSora_TW",
        salt: "DE7108E9B2842FD460F4777702727869",
        api: "https://api-launcher-tw.stargazer-games.com",
        pkg: "https://launcher-pkg-ss-hk.stargazer-games.com"
    },
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LauncherServerConfig {
    /// 启动器上报的游戏标识
    #[serde(rename = "TAG")]
    pub tag: String,

    /// 请求签名盐值
    #[serde(rename = "SALT")]
    pub salt: String,

    /// 配置接口域名
    #[serde(rename = "API")]
    pub api: String,

    /// 资源包域名
    #[serde(rename = "PKG")]
    pub pkg: String,
}

impl From<&StaticLauncherServer> for LauncherServerConfig {
    fn from(s: &StaticLauncherServer) -> Self {
        Self {
            tag: s.tag.into(),
            salt: s.salt.into(),
            api: s.api.into(),
            pkg: s.pkg.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherTaskConfig {
    /// 启动器版本号
    #[serde(rename = "LAUNCHER_VERSION")]
    pub version: String,

    /// 路由配置
    #[serde(rename = "ROUTE")]
    pub route: LauncherRouteConfig,

    /// 各区域启动器端点
    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, LauncherServerConfig>,

    /// 文件名
    #[serde(rename = "FILENAME")]
    pub filename: LauncherFileNameConfig,

    /// 输出目录
    #[serde(rename = "FILEPATH")]
    pub filepath: LauncherFilePathConfig,

    /// 资源注册表
    #[serde(rename = "REGISTRY")]
    pub registry: LauncherRegistryConfig,

    /// 版本号提取键
    #[serde(rename = "EXTRACTKEY")]
    pub extract_key: LauncherExtractKeyConfig,

    /// 开关
    #[serde(rename = "FLAGS")]
    pub flags: LauncherFlagsConfig,
}

impl Default for LauncherTaskConfig {
    fn default() -> Self {
        let mut servers = IndexMap::new();
        for (&region, server) in STATIC_LAUNCHER_SERVERS.entries() {
            servers.insert(region.to_string(), LauncherServerConfig::from(server));
        }

        Self {
            version: "1.6.0".into(),
            route: LauncherRouteConfig::default(),
            servers,
            filename: LauncherFileNameConfig::default(),
            filepath: LauncherFilePathConfig::default(),
            registry: LauncherRegistryConfig::default(),
            extract_key: LauncherExtractKeyConfig::default(),
            flags: LauncherFlagsConfig::default(),
        }
    }
}

impl LauncherTaskConfig {
    pub fn patterns(&self) -> Result<&[Regex]> {
        if let Some(cached) = self.registry.compiled.get() {
            return Ok(cached.as_slice());
        }

        let compiled = compile_patterns(&self.registry.res_patterns)?;
        Ok(self.registry.compiled.get_or_init(|| compiled).as_slice())
    }

    pub fn config_url(&self, server: &LauncherServerConfig) -> String {
        format!("{}{}", server.api.trim_end_matches('/'), self.route.config)
    }

    pub fn manifest_link_url(
        &self,
        server: &LauncherServerConfig,
        version: &str,
        file_path: &str,
    ) -> String {
        let params = self
            .route
            .params
            .replace("{ver}", version)
            .replace("{path}", file_path);

        format!("{}{}", self.config_url(server), params)
    }

    pub fn resource_url(server: &LauncherServerConfig, source: &str, path: &str) -> String {
        let source: &str = source.trim_matches('/');
        let path: &str = path.trim_start_matches('/');
        let base: &str = server.pkg.trim_end_matches('/');

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherRouteConfig {
    /// 配置接口路径
    #[serde(rename = "CONFIG")]
    pub config: String,

    /// 清单直链查询参数模板
    #[serde(rename = "PARAMS")]
    pub params: String,
}

impl Default for LauncherRouteConfig {
    fn default() -> Self {
        Self {
            config: "/api/launcher/game/config".into(),
            params: "/json?version={ver}&file_path={path}".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherFileNameConfig {
    /// launcher 清单文件名
    #[serde(rename = "MANIFEST")]
    pub manifest: String,
}

impl Default for LauncherFileNameConfig {
    fn default() -> Self {
        Self {
            manifest: "launcher_manifest.json".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherFilePathConfig {
    /// 资源输出目录
    #[serde(rename = "OUTPUT")]
    pub output: String,
}

impl Default for LauncherFilePathConfig {
    fn default() -> Self {
        Self {
            output: "./output/Launcher".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherRegistryConfig {
    /// 需要获取的文件列表
    #[serde(rename = "FETCH_FILES")]
    pub fetch_files: Vec<String>,

    /// 资源筛选正则
    #[serde(rename = "RES_PATTERNS")]
    pub res_patterns: Vec<String>,

    /// 编译后的正则 不参与序列化
    #[serde(skip)]
    compiled: Arc<OnceLock<Vec<Regex>>>,
}

impl Default for LauncherRegistryConfig {
    fn default() -> Self {
        Self {
            fetch_files: Vec::new(),
            res_patterns: vec![r"^data\.unity3d$".into()],
            compiled: Arc::new(OnceLock::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherExtractKeyConfig {
    /// 返回体里的版本字段名
    #[serde(rename = "LATEST_VERSION")]
    pub latest_version: String,

    /// 返回体里的路径字段名
    #[serde(rename = "LATEST_FILEPATH")]
    pub latest_filepath: String,
}

impl Default for LauncherExtractKeyConfig {
    fn default() -> Self {
        Self {
            latest_version: "game_latest_version".into(),
            latest_filepath: "game_latest_file_path".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherFlagsConfig {
    /// 保存类开关
    #[serde(rename = "SAVE")]
    pub save: LauncherSaveFlags,

    /// 运行类开关
    #[serde(rename = "OP")]
    pub op: LauncherOpFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSaveFlags {
    /// 是否保存原始清单
    #[serde(rename = "RAW_MANIFEST")]
    pub raw_manifest: bool,

    /// 是否保存版本文件
    #[serde(rename = "VERSION_FILE")]
    pub version_file: bool,
}

impl Default for LauncherSaveFlags {
    fn default() -> Self {
        Self {
            raw_manifest: false,
            version_file: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherOpFlags {
    /// 是否下载资源文件
    #[serde(rename = "DOWNLOAD_RESOURCE")]
    pub download_resource: bool,

    /// 是否覆盖资源文件
    #[serde(rename = "OVERWRITE_RESOURCE")]
    pub overwrite_resource: bool,

    /// 是否过滤过时补丁
    #[serde(rename = "FILTER_OUTDATED_PATCH")]
    pub filter_outdated_patch: bool,

    /// 是否使用默认区域
    #[serde(rename = "USE_DEFAULT_REGION")]
    pub use_default_region: bool,

    /// 是否启用资源正则过滤
    #[serde(rename = "USE_RESOURCE_REGEX")]
    pub use_resource_regex: bool,
}

impl Default for LauncherOpFlags {
    fn default() -> Self {
        Self {
            download_resource: true,
            overwrite_resource: false,
            filter_outdated_patch: true,
            use_default_region: true,
            use_resource_regex: false,
        }
    }
}

pub fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    let mut compiled = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        let regex = Regex::new(pattern).map_err(|err| {
            AppError::message(format!("invalid launcher regex '{pattern}': {err}"))
        })?;
        compiled.push(regex);
    }

    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::{LauncherServerConfig, LauncherTaskConfig, compile_patterns};

    fn launcher_server() -> LauncherServerConfig {
        LauncherServerConfig {
            tag: "StellaSora_KR".into(),
            salt: "salt".into(),
            api: "https://api.example.com/".into(),
            pkg: "https://pkg.example.com".into(),
        }
    }

    #[test]
    fn builds_launcher_config_urls() {
        let config = LauncherTaskConfig::default();
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
            LauncherTaskConfig::resource_url(&server, "/game-1.2.3", "/a/b/data.unity3d"),
            "https://pkg.example.com/game-1.2.3/a/b/data.unity3d"
        );
    }

    #[test]
    fn launcher_patterns_are_compiled_once() {
        let config = LauncherTaskConfig::default();
        let first = config.patterns().expect("patterns should compile");
        let second = config.patterns().expect("patterns should compile");

        assert!(std::ptr::eq(first, second));
        assert!(first.iter().any(|regex| regex.is_match("data.unity3d")));
    }

    #[test]
    fn invalid_launcher_pattern_is_reported() {
        let err = compile_patterns(&["(".into()]).unwrap_err();
        assert!(err.to_string().contains("invalid launcher regex"));
    }

    #[test]
    fn launcher_defaults_cover_all_builtin_regions() {
        let config = LauncherTaskConfig::default();

        assert_eq!(config.servers.len(), 5);
        assert!(config.servers.contains_key("CN"));
    }
}
