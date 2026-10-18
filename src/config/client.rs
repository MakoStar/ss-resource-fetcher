use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::{resolve_platform_list, resolve_platform_value};

pub const NEW_FILE_PATCH_TAG: &str = "n";

pub const DELTA_PATCH_TAG: &str = "u";

pub const MANIFEST_PATCH_TAG: &str = "m";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientTaskConfig {
    /// 路由配置
    #[serde(rename = "ROUTE")]
    pub route: ClientRouteConfig,

    /// 各区域资源服务器
    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, ClientServerConfig>,

    /// 文件名（平台相关）
    #[serde(rename = "FILENAME")]
    pub filename: ClientFileNameConfig,

    /// 输出目录
    #[serde(rename = "FILEPATH")]
    pub filepath: ClientFilePathConfig,

    /// 资源注册表
    #[serde(rename = "REGISTRY")]
    pub registry: ClientRegistryConfig,

    /// 版本号提取键
    #[serde(rename = "EXTRACTKEY")]
    pub extract_key: ClientExtractKeyConfig,

    /// 开关
    #[serde(rename = "FLAGS")]
    pub flags: ClientFlagsConfig,
}

impl Default for ClientTaskConfig {
    fn default() -> Self {
        let mut servers = IndexMap::new();
        for (&region, server) in STATIC_CLIENT_SERVERS.entries() {
            servers.insert(region.to_string(), ClientServerConfig::from(server));
        }

        Self {
            route: ClientRouteConfig::default(),
            servers,
            filename: ClientFileNameConfig::default(),
            filepath: ClientFilePathConfig::default(),
            registry: ClientRegistryConfig::default(),
            extract_key: ClientExtractKeyConfig::default(),
            flags: ClientFlagsConfig::default(),
        }
    }
}

impl ClientTaskConfig {
    pub fn resolve(&mut self, platform: &str) {
        let template = Self::default();

        self.route.manifest =
            resolve_platform_value(&self.route.manifest, &template.route.manifest, platform);
        self.route.resource =
            resolve_platform_value(&self.route.resource, &template.route.resource, platform);

        self.filename.raw_manifest = resolve_platform_value(
            &self.filename.raw_manifest,
            &template.filename.raw_manifest,
            platform,
        );
        self.filename.decrypted_manifest = resolve_platform_value(
            &self.filename.decrypted_manifest,
            &template.filename.decrypted_manifest,
            platform,
        );
        self.filename.decoded_manifest = resolve_platform_value(
            &self.filename.decoded_manifest,
            &template.filename.decoded_manifest,
            platform,
        );
        self.filename.patch_manifest = resolve_platform_value(
            &self.filename.patch_manifest,
            &template.filename.patch_manifest,
            platform,
        );
        self.filename.version =
            resolve_platform_value(&self.filename.version, &template.filename.version, platform);
        self.filename.root_mani = resolve_platform_value(
            &self.filename.root_mani,
            &template.filename.root_mani,
            platform,
        );
        self.filename.first_mani_patch = resolve_platform_value(
            &self.filename.first_mani_patch,
            &template.filename.first_mani_patch,
            platform,
        );
        self.filename.add_mani = resolve_platform_value(
            &self.filename.add_mani,
            &template.filename.add_mani,
            platform,
        );

        self.registry.resolve(platform);
    }

    pub fn link_updatetag(&mut self, patch_prefix: &str, patch_tags: &[String]) {
        self.registry.patch_prefix = patch_prefix.to_string();
        self.registry.patch_tags = patch_tags.to_vec();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientRouteConfig {
    /// 清单路由（平台相关）
    #[serde(rename = "MANIFEST")]
    pub manifest: String,

    /// 资源路由（平台相关）
    #[serde(rename = "RESOURCE")]
    pub resource: String,
}

impl Default for ClientRouteConfig {
    fn default() -> Self {
        Self {
            manifest: "/meta/{platform}.html".into(),
            resource: "/res/{platform}/".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StaticClientServer {
    /// 服务器地址
    pub url: &'static str,
    /// 解密密钥
    pub key: &'static str,
}

pub static STATIC_CLIENT_SERVERS: phf::OrderedMap<&'static str, StaticClientServer> = phf::phf_ordered_map! {
    "CN" => StaticClientServer { url: "https://nova-static.yostar.cn", key: "Xf&FRcsYm48cJ2A@" },
    "EN" => StaticClientServer { url: "https://nova-static.stellasora.global", key: "ma5Dn2FhC*Xhxy%c" },
    "JP" => StaticClientServer { url: "https://nova-static.stellasora.jp", key: "ZnUFA@S9%4KyoryM" },
    "KR" => StaticClientServer { url: "https://nova-static.stellasora.kr", key: "U9cjHuwGDDx&$drn" },
    "TW" => StaticClientServer { url: "https://nova-static.stargazer-games.com", key: "owGYVDmfHrxi^4pm" },
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientServerConfig {
    /// 服务器地址
    #[serde(rename = "URL")]
    pub url: String,

    /// 解密密钥
    #[serde(rename = "KEY")]
    pub key: String,
}

impl From<&StaticClientServer> for ClientServerConfig {
    fn from(s: &StaticClientServer) -> Self {
        Self {
            url: s.url.into(),
            key: s.key.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientFileNameConfig {
    /// 原始清单文件名
    #[serde(rename = "RAW_MANIFEST")]
    pub raw_manifest: String,

    /// 解密后的清单文件名
    #[serde(rename = "DECRYPTED_MANIFEST")]
    pub decrypted_manifest: String,

    /// 解码后的清单文件名
    #[serde(rename = "DECODED_MANIFEST")]
    pub decoded_manifest: String,

    /// 补丁清单文件名
    #[serde(rename = "PATCH_MANIFEST")]
    pub patch_manifest: String,

    /// 版本文件名
    #[serde(rename = "VERSION")]
    pub version: String,

    /// 更新根清单文件名
    #[serde(rename = "ROOT_MANI")]
    pub root_mani: String,

    /// 第一个清单补丁文件名
    #[serde(rename = "FIRST_MANI_PATCH")]
    pub first_mani_patch: String,

    /// 附加清单文件名
    #[serde(rename = "ADD_MANI")]
    pub add_mani: String,
}

impl Default for ClientFileNameConfig {
    fn default() -> Self {
        Self {
            raw_manifest: "{platform}.html".into(),
            decrypted_manifest: "{platform}.bytes".into(),
            decoded_manifest: "{platform}_resource_manifest.json".into(),
            patch_manifest: "{platform}_patch_manifest.json".into(),
            version: "{platform}_version.json".into(),
            root_mani: "ss_{platform}.mani".into(),
            first_mani_patch: "p_1_m.ss_{platform}.mani".into(),
            add_mani: "ss_{platform}_add.mani".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientFilePathConfig {
    /// 根输出目录
    #[serde(rename = "ROOT_OUTPUT")]
    pub root_output: String,

    /// 清单文件输出目录
    #[serde(rename = "MANIFEST")]
    pub manifest: String,

    /// 版本文件输出目录
    #[serde(rename = "VERSIONS")]
    pub versions: String,

    /// 补丁文件输出目录
    #[serde(rename = "PATCH")]
    pub patch: String,

    /// 解包文件输出目录
    #[serde(rename = "UNPACK")]
    pub unpack: String,
}

impl Default for ClientFilePathConfig {
    fn default() -> Self {
        Self {
            root_output: "./output/".into(),
            manifest: "./output/Manifest".into(),
            versions: "./output/Manifest".into(),
            patch: "./output/Metadata".into(),
            unpack: "./output/Unpack".into(),
        }
    }
}

#[derive(Debug, Default)]
struct CompiledRegistry {
    full_patterns: Vec<Regex>,
    suffix_patterns: Vec<Regex>,
    patch_tag_patterns: Vec<Regex>,
    fetch_names: HashSet<String>,
    fetch_extensions: Vec<String>,
    unpack_names: HashSet<String>,
}

impl CompiledRegistry {
    fn build(registry: &ClientRegistryConfig) -> Self {
        let mut full_patterns = Vec::with_capacity(registry.res_patterns.len());
        let mut suffix_patterns = Vec::with_capacity(registry.res_patterns.len());

        for pattern in &registry.res_patterns {
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

        let mut patch_tag_patterns = Vec::with_capacity(registry.patch_tags.len());
        for tag in &registry.patch_tags {
            let pattern = format!("{prefix}[0-9]+_{tag}\\.", prefix = registry.patch_prefix);
            match Regex::new(&pattern) {
                Ok(regex) => patch_tag_patterns.push(regex),
                Err(err) => log::warn!("Invalid patch tag regex '{pattern}': {err}"),
            }
        }

        let fetch_names = registry.fetch_files.iter().cloned().collect();

        let fetch_extensions = registry
            .fetch_files
            .iter()
            .map(|name| format!(".{name}"))
            .collect();

        let unpack_names = registry.unpack_files.iter().cloned().collect();

        Self {
            full_patterns,
            suffix_patterns,
            patch_tag_patterns,
            fetch_names,
            fetch_extensions,
            unpack_names,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientRegistryConfig {
    /// 需要获取的文件列表
    #[serde(rename = "FETCH_FILES")]
    pub fetch_files: Vec<String>,

    /// 需要解包的文件列表
    #[serde(rename = "UNPACK_FILES")]
    pub unpack_files: Vec<String>,

    /// 基础资源正则模式
    #[serde(rename = "RES_PATTERNS")]
    pub res_patterns: Vec<String>,

    /// 补丁标签前缀 加载时从 PATCH_UPDATE_TAG 链接进来 不参与序列化
    #[serde(skip)]
    pub patch_prefix: String,

    /// 纳入下载的资源补丁标签 加载时从 PATCH_UPDATE_TAG 链接进来 不参与序列化
    #[serde(skip)]
    pub patch_tags: Vec<String>,

    /// 编译后的注册表 不参与序列化
    #[serde(skip)]
    compiled: OnceLock<CompiledRegistry>,
}

impl Clone for ClientRegistryConfig {
    fn clone(&self) -> Self {
        Self {
            fetch_files: self.fetch_files.clone(),
            unpack_files: self.unpack_files.clone(),
            res_patterns: self.res_patterns.clone(),
            patch_prefix: self.patch_prefix.clone(),
            patch_tags: self.patch_tags.clone(),
            compiled: OnceLock::new(),
        }
    }
}

impl ClientRegistryConfig {
    fn compiled(&self) -> &CompiledRegistry {
        self.compiled.get_or_init(|| CompiledRegistry::build(self))
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

    pub fn is_patch_resource(&self, file_name: &str) -> bool {
        self.compiled()
            .patch_tag_patterns
            .iter()
            .any(|regex| regex.is_match(file_name))
    }

    pub fn patch_tag<'a>(&self, file_name: &'a str) -> Option<&'a str> {
        let rest = file_name.strip_prefix(&self.patch_prefix)?;
        let tail = rest.split_once('.')?.0;

        Some(tail.split_once('_')?.1)
    }

    pub fn patch_sequence(&self, file_name: &str) -> u64 {
        let rest = file_name
            .strip_prefix(&self.patch_prefix)
            .unwrap_or(file_name);

        rest.split_once('_')
            .and_then(|(sequence, _)| sequence.parse().ok())
            .unwrap_or_default()
    }

    pub fn patch_base_name<'a>(&self, file_name: &'a str) -> Option<&'a str> {
        let rest = file_name.strip_prefix(&self.patch_prefix)?;
        let (patch, base) = rest.split_once('.')?;

        if !patch
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit())
        {
            return None;
        }

        Some(base)
    }

    pub fn is_new_file_patch(&self, file_name: &str) -> bool {
        self.patch_tag(file_name) == Some(NEW_FILE_PATCH_TAG)
    }

    pub fn is_fetchable(&self, file_name: &str) -> bool {
        let compiled = self.compiled();

        compiled.fetch_names.contains(file_name)
            || compiled
                .fetch_extensions
                .iter()
                .any(|extension| file_name.ends_with(extension.as_str()))
            || self.is_patch_resource(file_name)
    }
}

impl Default for ClientRegistryConfig {
    fn default() -> Self {
        Self {
            fetch_files: vec![
                "lua.arcx".into(),
                "data.arcx".into(),
                "lua.json".into(),
                "data.json".into(),
                "ss_{platform}.mani".into(),
                "ss_{platform}_add.mani".into(),
            ],
            unpack_files: vec!["data.arcx".into(), "lua.arcx".into()],
            res_patterns: vec![
                r"^ss_{platform}(_add)?\.mani$".into(),
                r"^icon-[0-9A-Za-z]+\.unity3d$".into(),
            ],
            patch_prefix: "p_".into(),
            patch_tags: vec![DELTA_PATCH_TAG.into()],
            compiled: OnceLock::new(),
        }
    }
}

impl ClientRegistryConfig {
    pub fn resolve(&mut self, platform: &str) {
        let template = Self::default();

        self.res_patterns =
            resolve_platform_list(&self.res_patterns, &template.res_patterns, platform);
        self.fetch_files =
            resolve_platform_list(&self.fetch_files, &template.fetch_files, platform);
        self.unpack_files =
            resolve_platform_list(&self.unpack_files, &template.unpack_files, platform);
        self.compiled = OnceLock::new();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientExtractKeyConfig {
    /// 默认版本键值
    #[serde(rename = "DEFAULT")]
    pub default: String,

    /// 客户端版本键
    #[serde(rename = "CLIENT")]
    pub client: String,

    /// 游戏版本键
    #[serde(rename = "GAME")]
    pub game: String,

    /// 补丁版本键
    #[serde(rename = "PATCH")]
    pub patch: String,
}

impl Default for ClientExtractKeyConfig {
    fn default() -> Self {
        Self {
            default: "unknown".into(),
            client: "CLIENT_VER".into(),
            game: "GAME_VER".into(),
            patch: "BIN_DIFF_PATCH_VER".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientFlagsConfig {
    /// 保存类开关
    #[serde(rename = "SAVE")]
    pub save: ClientSaveFlags,

    /// 运行类开关
    #[serde(rename = "OP")]
    pub op: ClientOpFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientSaveFlags {
    /// 是否保存原始清单
    #[serde(rename = "RAW_MANIFEST")]
    pub raw_manifest: bool,

    /// 是否保存解密后的清单
    #[serde(rename = "DECRYPTED_MANIFEST")]
    pub decrypted_manifest: bool,

    /// 是否保存解码后的清单
    #[serde(rename = "DECODED_MANIFEST")]
    pub decoded_manifest: bool,

    /// 是否保存补丁清单
    #[serde(rename = "PATCH_MANIFEST")]
    pub patch_manifest: bool,

    /// 是否保存版本文件
    #[serde(rename = "VERSION_FILE")]
    pub version_file: bool,
}

impl Default for ClientSaveFlags {
    fn default() -> Self {
        Self {
            raw_manifest: false,
            decrypted_manifest: false,
            decoded_manifest: true,
            patch_manifest: true,
            version_file: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientOpFlags {
    /// 是否下载资源文件
    #[serde(rename = "DOWNLOAD_RESOURCE")]
    pub download_resource: bool,

    /// 是否覆盖资源文件
    #[serde(rename = "OVERWRITE_RESOURCE")]
    pub overwrite_resource: bool,

    /// 是否合并补丁资源
    #[serde(rename = "MERGE_PATCH")]
    pub merge_patch: bool,

    /// 是否提取补丁清单
    #[serde(rename = "EXTRACT_PATCH_MANIFEST")]
    pub extract_patch_manifest: bool,

    /// 是否提取补丁版本信息
    #[serde(rename = "EXTRACT_PATCH_VERSION")]
    pub extract_patch_version: bool,

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

impl Default for ClientOpFlags {
    fn default() -> Self {
        Self {
            download_resource: true,
            overwrite_resource: false,
            merge_patch: true,
            extract_patch_manifest: true,
            extract_patch_version: true,
            filter_outdated_patch: true,
            use_default_region: true,
            use_resource_regex: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_substitutes_platform() {
        let mut client = ClientTaskConfig::default();
        client.resolve("and");

        assert_eq!(client.route.manifest, "/meta/and.html");
        assert_eq!(client.route.resource, "/res/and/");
        assert_eq!(client.filename.raw_manifest, "and.html");
        assert_eq!(client.filename.root_mani, "ss_and.mani");
    }

    #[test]
    fn resolve_switches_platform_when_default_changed() {
        let mut client = ClientTaskConfig::default();
        client.resolve("win");
        client.resolve("and");

        assert_eq!(client.route.manifest, "/meta/and.html");
        assert_eq!(client.route.resource, "/res/and/");
        assert_eq!(client.filename.root_mani, "ss_and.mani");
        assert!(
            client
                .registry
                .fetch_files
                .iter()
                .any(|name| name == "ss_and.mani")
        );
        assert!(
            client
                .registry
                .res_patterns
                .iter()
                .any(|pattern| pattern == r"^ss_and(_add)?\.mani$")
        );
    }

    #[test]
    fn resolve_keeps_a_custom_value() {
        let mut client = ClientTaskConfig::default();
        client.filename.root_mani = "my_own.mani".into();
        client.resolve("and");

        assert_eq!(client.filename.root_mani, "my_own.mani");
    }

    #[test]
    fn registry_matching_after_link() {
        let mut client = ClientTaskConfig::default();
        client.resolve("and");
        client.link_updatetag("p_", &["u".to_string()]);

        assert!(client.registry.is_base_resource("ss_and.mani"));
        assert!(client.registry.is_patch_resource("p_12_u.lua.arcx"));
        assert!(client.registry.is_fetchable("ss_and.mani"));
    }
}
