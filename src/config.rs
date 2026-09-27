use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use network_manager::FetcherConfig;
use regex::Regex;

use crate::proto::FileDiff;

// ============================================================
// 默认值常量
// ============================================================
pub const CONFIG_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

// ============================================================
// 服务器资源路由配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerRouteConfigs {
    /// 清单文件的 API 路径
    #[serde(rename = "MANIFEST_ROUTE")]
    pub manifest_route: String,
    /// 资源文件的 API 路径
    #[serde(rename = "RESOURCE_ROUTE")]
    pub resource_route: String,
}

impl Default for ServerRouteConfigs {
    fn default() -> Self {
        Self {
            manifest_route: "/meta/win.html".into(),
            resource_route: "/res/win/".into(),
        }
    }
}

// ============================================================
// 文件相关的路径和名称配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilePathConfigs {
    /// 根输出目录
    #[serde(rename = "ROOT_OUTPUT_DIR")]
    pub root_output_dir: String,
    /// 清单文件输出目录
    #[serde(rename = "MANIFEST_OUTPUT_DIR")]
    pub manifest_output_dir: String,
    /// 版本文件输出目录
    #[serde(rename = "VERSIONS_OUTPUT_DIR")]
    pub versions_output_dir: String,
    /// 补丁文件输出目录
    #[serde(rename = "PATCH_OUTPUT_DIR")]
    pub patch_output_dir: String,
    /// 解包文件输出目录
    #[serde(rename = "UNPACK_OUTPUT_DIR")]
    pub unpack_output_dir: String,
}

impl Default for FilePathConfigs {
    fn default() -> Self {
        Self {
            root_output_dir: "./output/".into(),
            manifest_output_dir: "./output/Manifest".into(),
            versions_output_dir: "./output/Manifest".into(),
            patch_output_dir: "./output/Metadata".into(),
            unpack_output_dir: "./output/Unpack".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileNameConfigs {
    /// 原始清单文件名
    #[serde(rename = "MANIFEST_RAW_FILE")]
    pub manifest_raw_file: String,
    /// 解密后的清单文件名
    #[serde(rename = "MANIFEST_DECRYPT_FILE")]
    pub manifest_decrypt_file: String,
    /// 解码后的清单文件名
    #[serde(rename = "MANIFEST_DECODED_FILE")]
    pub manifest_decoded_file: String,
    /// 补丁清单文件名
    #[serde(rename = "PATCH_MANIFEST_FILE")]
    pub patch_manifest_file: String,
    /// 版本文件名
    #[serde(rename = "VERSION_FILE")]
    pub version_file: String,
    /// 更新根清单文件名
    #[serde(rename = "UPDATE_ROOT_MANI_FILE")]
    pub update_root_mani_file: String,
    /// 资源补丁标签前缀
    #[serde(rename = "UPDATE_RESOURCE_PATCH_TAG_PREFIX")]
    pub update_resource_patch_tag_prefix: String,
    /// 资源补丁标签后缀
    #[serde(rename = "UPDATE_RESOURCE_PATCH_TAG_SUFFIX")]
    pub update_resource_patch_tag_suffix: String,
    /// 清单补丁标签后缀
    #[serde(rename = "UPDATE_MANIFEST_PATCH_TAG_SUFFIX")]
    pub update_manifest_patch_tag_suffix: String,
    /// 清单扩展名
    #[serde(rename = "UPDATE_MANIFEST_EXTENSION")]
    pub update_manifest_extension: String,
    /// 第一个清单补丁文件名
    #[serde(rename = "UPDATE_FIRST_MANIFEST_PATCH_NAME")]
    pub update_first_manifest_patch_name: String,
    /// 附加清单文件名
    #[serde(rename = "UPDATE_ADD_MANIFEST_NAME")]
    pub update_add_manifest_name: String,
}

impl Default for FileNameConfigs {
    fn default() -> Self {
        Self {
            manifest_raw_file: "win.html".into(),
            manifest_decrypt_file: "win.bytes".into(),
            manifest_decoded_file: "resource_manifest.json".into(),
            patch_manifest_file: "patch_manifest.json".into(),
            version_file: "version.json".into(),
            update_root_mani_file: "ss_win.mani".into(),
            update_resource_patch_tag_prefix: "p_".into(),
            update_resource_patch_tag_suffix: "_u".into(),
            update_manifest_patch_tag_suffix: "_m".into(),
            update_manifest_extension: ".mani".into(),
            update_first_manifest_patch_name: "p_1_m.ss_win.mani".into(),
            update_add_manifest_name: "ss_win_add.mani".into(),
        }
    }
}

// ============================================================
// 提取器模块相关的配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractorConfig {
    /// 默认版本键值
    #[serde(rename = "DEFAULT_VERSION_KEY")]
    pub default_version_key: String,
    /// 客户端版本键
    #[serde(rename = "CLIENT_VERSION_KEY")]
    pub client_version_key: String,
    /// 游戏版本键
    #[serde(rename = "GAME_VERSION_KEY")]
    pub game_version_key: String,
    /// 补丁版本键
    #[serde(rename = "PATCH_VERSION_KEY")]
    pub patch_version_key: String,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            default_version_key: "unknown".into(),
            client_version_key: "CLIENT_VER".into(),
            game_version_key: "GAME_VER".into(),
            patch_version_key: "BIN_DIFF_PATCH_VER".into(),
        }
    }
}

// ============================================================
// 发送请求相关的配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RequestConfigs {
    /// 是否验证 TLS 证书
    #[serde(rename = "VERIFY_TLS")]
    pub verify_tls: bool,
    /// 请求超时时间（秒）
    #[serde(rename = "TIMEOUT_SECS")]
    pub timeout_secs: u64,
    /// 总重试次数
    #[serde(rename = "TOTAL_RETRIES")]
    pub total_retries: u32,
    /// 退避因子
    #[serde(rename = "BACKOFF_FACTOR")]
    pub backoff_factor: u32,
    /// 连接池最大空闲连接数
    #[serde(rename = "POOL_MAX_IDLE")]
    pub pool_max_idle: usize,
    /// 最小重试延迟（毫秒）
    #[serde(rename = "MIN_RETRY_DELAY_MS")]
    pub min_retry_delay_ms: u64,
    /// 最大重试延迟（秒）
    #[serde(rename = "MAX_RETRY_DELAY_SECS")]
    pub max_retry_delay_secs: u64,
    /// HTTP 额外请求头
    #[serde(rename = "EXTRA_HEADERS")]
    pub extra_headers: HashMap<String, String>,
}

impl RequestConfigs {
    pub fn build_fetcher_config(&self) -> FetcherConfig {
        FetcherConfig::builder()
            .timeout(self.timeout_secs)
            .max_retries(self.total_retries)
            .retry_bounds(
                Duration::from_millis(self.min_retry_delay_ms),
                Duration::from_secs(self.max_retry_delay_secs),
            )
            .verify_tls(self.verify_tls)
            .pool_max_idle(self.pool_max_idle)
            .headers(self.extra_headers.clone())
            .build()
    }
}

impl Default for RequestConfigs {
    fn default() -> Self {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Accept-Encoding".into(), "identity".into());
        headers.insert("X-Unity-Version".into(), "2022.3.62f2".into());
        headers.insert(
            "User-Agent".into(),
            "UnityPlayer/2022.3.62f2 (UnityWebRequest/1.0, libcurl/8.10.1-DEV)".into(),
        );
        Self {
            verify_tls: false,
            timeout_secs: 180,
            total_retries: 2,
            backoff_factor: 3,
            pool_max_idle: 50,
            min_retry_delay_ms: 500,
            max_retry_delay_secs: 30,
            extra_headers: headers,
        }
    }
}

// ============================================================
// 需要用到的资源名称注册配置
// ============================================================
fn base_resource_regexes(patterns: &[String]) -> &'static [Regex] {
    static CACHE: OnceLock<Vec<Regex>> = OnceLock::new();
    CACHE.get_or_init(|| {
        patterns
            .iter()
            .filter_map(|p| match Regex::new(p) {
                Ok(re) => Some(re),
                Err(e) => {
                    log::warn!("Invalid resource regex '{}': {}", p, e);
                    None
                }
            })
            .collect()
    })
}

fn base_resource_suffix_regexes(patterns: &[String]) -> &'static [Regex] {
    static CACHE: OnceLock<Vec<Regex>> = OnceLock::new();
    CACHE.get_or_init(|| {
        patterns
            .iter()
            .filter_map(|p| {
                let suffix_pat: &str = p.strip_prefix('^').unwrap_or(p);
                match Regex::new(suffix_pat) {
                    Ok(re) => Some(re),
                    Err(e) => {
                        log::warn!("Invalid suffix regex '{}': {}", suffix_pat, e);
                        None
                    }
                }
            })
            .collect()
    })
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
}

impl ResourceRegistryConfig {
    #[allow(dead_code)]
    pub fn need_fetch_set(&self) -> HashSet<&str> {
        self.need_fetch_files.iter().map(|s| s.as_str()).collect()
    }

    #[allow(dead_code)]
    pub fn fetchable_extensions(&self) -> Vec<String> {
        self.need_fetch_files
            .iter()
            .map(|s| format!(".{}", s))
            .collect()
    }

    #[allow(dead_code)]
    pub fn unpack_set(&self) -> HashSet<&str> {
        self.unpack_files.iter().map(|s| s.as_str()).collect()
    }

    #[allow(dead_code)]
    pub fn is_base_resource(&self, name: &str) -> bool {
        base_resource_regexes(&self.base_resource_patterns)
            .iter()
            .any(|re| re.is_match(name))
    }

    #[allow(dead_code)]
    pub fn is_base_resource_suffix(&self, name: &str) -> bool {
        base_resource_suffix_regexes(&self.base_resource_patterns)
            .iter()
            .any(|re| re.is_match(name))
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
        }
    }
}

// ============================================================
// 服务器相关的配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigOwned {
    #[serde(rename = "URL")]
    pub url: String,
    #[serde(rename = "KEY")]
    pub key: String,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticServerConfig {
    pub url: &'static str,
    pub key: &'static str,
}

static STATIC_RESOURCE_SERVERS: phf::OrderedMap<&'static str, StaticServerConfig> = phf::phf_ordered_map! {
    "CN" => StaticServerConfig { url: "https://nova-static.yostar.cn", key: "Xf&FRcsYm48cJ2A@" },
    "EN" => StaticServerConfig { url: "https://nova-static.stellasora.global", key: "ma5Dn2FhC*Xhxy%c" },
    "JP" => StaticServerConfig { url: "https://nova-static.stellasora.jp", key: "ZnUFA@S9%4KyoryM" },
    "KR" => StaticServerConfig { url: "https://nova-static.stellasora.kr", key: "U9cjHuwGDDx&$drn" },
    "TW" => StaticServerConfig { url: "https://nova-static.stargazer-games.com", key: "owGYVDmfHrxi^4pm" },
};

static SERVER_ORDER: &[&str] = &["CN", "EN", "JP", "KR", "TW"];

impl From<&StaticServerConfig> for ServerConfigOwned {
    fn from(s: &StaticServerConfig) -> Self {
        Self {
            url: s.url.into(),
            key: s.key.into(),
        }
    }
}

// ============================================================
// 功能开关
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeatureFlags {
    /// 是否使用默认区域键
    #[serde(rename = "IS_USE_DEFAULT_REGION")]
    pub is_use_default_region: bool,
    /// 是否保存原始清单
    #[serde(rename = "IS_SAVE_RAW_MANIFEST")]
    pub is_save_raw_manifest: bool,
    /// 是否保存解密后的清单
    #[serde(rename = "IS_SAVE_DECRYPTED_MANIFEST")]
    pub is_save_decrypted_manifest: bool,
    /// 是否保存解码后的清单
    #[serde(rename = "IS_SAVE_DECODED_MANIFEST")]
    pub is_save_decoded_manifest: bool,
    /// 是否提取补丁清单
    #[serde(rename = "IS_EXTRACT_PATCH_MANIFEST")]
    pub is_extract_patch_manifest: bool,
    /// 是否保存补丁清单
    #[serde(rename = "IS_SAVE_PATCH_MANIFEST")]
    pub is_save_patch_manifest: bool,
    /// 是否下载资源文件
    #[serde(rename = "IS_DOWNLOAD_RESOURCE")]
    pub is_download_resource: bool,
    /// 是否覆盖资源文件
    #[serde(rename = "IS_OVERWRITE_RESOURCE")]
    pub is_overwrite_resource: bool,
    /// 是否合并补丁资源
    #[serde(rename = "IS_MERGE_PATCH")]
    pub is_merge_patch: bool,
    /// 是否保存版本文件
    #[serde(rename = "IS_SAVE_VERSION_FILE")]
    pub is_save_version_file: bool,
    /// 是否提取补丁版本信息
    #[serde(rename = "IS_EXTRACT_PATCH_VERSION")]
    pub is_extract_patch_version: bool,
    /// 是否启用资源正则过滤
    #[serde(rename = "IS_USE_RESOURCE_REGEX")]
    pub is_use_resource_regex: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            is_use_default_region: true,
            is_save_raw_manifest: false,
            is_save_decrypted_manifest: false,
            is_save_decoded_manifest: true,
            is_extract_patch_manifest: true,
            is_save_patch_manifest: true,
            is_download_resource: true,
            is_overwrite_resource: false,
            is_merge_patch: true,
            is_save_version_file: true,
            is_extract_patch_version: true,
            is_use_resource_regex: true,
        }
    }
}

// ============================================================
// 应用程序主配置
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 默认区域：CN / EN / JP / KR / TW
    #[serde(rename = "DEFAULT_REGION")]
    pub default_region: String,

    #[serde(rename = "SERVER_ROUTE")]
    pub server_route: ServerRouteConfigs,

    #[serde(rename = "FILE_PATH")]
    pub file_path: FilePathConfigs,

    #[serde(rename = "FILE_NAME")]
    pub file_name: FileNameConfigs,

    #[serde(rename = "EXTRACTOR")]
    pub extractor: ExtractorConfig,

    #[serde(rename = "REQUEST")]
    pub request: RequestConfigs,

    #[serde(rename = "RESOURCE_REGISTRY")]
    pub resource_registry: ResourceRegistryConfig,

    #[serde(rename = "SERVERS")]
    pub servers: IndexMap<String, ServerConfigOwned>,

    #[serde(rename = "FEATURE_FLAGS")]
    pub feature_flags: FeatureFlags,
}

impl AppConfig {
    /// 获取全局配置实例
    #[inline(always)]
    pub fn get() -> &'static AppConfig {
        &APP_CONFIG
    }

    /// 获取所有可用区域
    pub fn available_regions(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(|s| s.as_str())
    }

    /// 获取所有服务器配置
    #[allow(dead_code)]
    pub fn all_server_configs(&self) -> impl Iterator<Item = (&str, &ServerConfigOwned)> {
        self.servers.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// 获取服务器配置
    pub fn get_server(&self, region: &str) -> Option<&ServerConfigOwned> {
        self.servers.get(region)
    }

    /// 构建 Fetcher 配置
    pub fn build_fetcher_config(&self) -> FetcherConfig {
        self.request.build_fetcher_config()
    }

    /// 保存配置到 TOML 文件
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

    /// 生成带注释的 TOML 字符串（纯字符串操作）
    fn to_toml_with_comments(&self) -> String {
        let mut content: String =
            toml::to_string_pretty(self).expect("failed to serialize AppConfig");
        content = self.add_all_comments(&content);

        content
    }

    /// 添加所有注释
    fn add_all_comments(&self, content: &str) -> String {
        let mut result: String = content.to_string();
        let header: String = format!(
            "# {}\n{} \n{} \n# {} \n\n",
            "=".repeat(60),
            "# 应用程序配置文件",
            "# 修改后重启应用即可生效",
            "=".repeat(60),
        );
        result = format!("{}{}", header, result);

        if let Some(pos) = result.find("DEFAULT_REGION = ") {
            let comment: &str = "# 默认区域: 启用 IS_USE_DEFAULT_REGION 时生效\n";
            let before: &str = &result[..pos];
            let after: &str = &result[pos..];
            result = format!("{}{}{}", before, comment, after);
        }

        let sections: [(&str, &str, &str, &str); 8] = [
            ("[SERVER_ROUTE]", "服务器路由配置", "", ""),
            (
                "[FILE_PATH]",
                "文件路径配置",
                "注意：所有路径都是相对于程序运行目录",
                "",
            ),
            ("[FILE_NAME]", "文件名配置", "", ""),
            (
                "[EXTRACTOR]",
                "提取器配置",
                "当前应用于 .mani 文件 version 提取 key (DO NOT EDIT)",
                "",
            ),
            (
                "[REQUEST]",
                "HTTP 请求配置",
                "如果网络不好或者文件太大, 请求读取 body 会超时",
                "可适当增加 TIMEOUT_SECS 来避免(特别是 lua.arcx )",
            ),
            ("[RESOURCE_REGISTRY]", "资源注册表配置", "", ""),
            (
                "[SERVERS.",
                "服务器配置: ",
                "区域名 = { URL = \"服务器地址\", KEY = \"密钥\" }",
                "KEY: 来自于提取的 ClientConfig.json 的 serverMetaKey ",
            ),
            (
                "[FEATURE_FLAGS]",
                "功能开关",
                "注意: IS_SAVE_PATCH_MANIFEST 必须是 true",
                "因为合并补丁需要读取补丁清单和校验清单中的 hash",
            ),
        ];

        for (section, title, note, next) in sections {
            if let Some(pos) = result.find(section) {
                let mut comment: String = format!("\n# {}\n# {}\n", "=".repeat(60), title,);
                if !note.is_empty() {
                    comment = format!("{}# {}\n", comment, note);
                }
                if !next.is_empty() {
                    comment = format!("{}# {}\n", comment, next);
                }
                comment = format!("{}# {}\n", comment, "=".repeat(60),);
                let before: &str = &result[..pos];
                let after: &str = &result[pos..];
                result = format!("{}{}{}", before, comment, after);
            }
        }

        // 找到 SERVERS 部分的每一行，添加注释 (可能会用到)
        let lines: Vec<String> = result.lines().map(|s| s.to_string()).collect();
        let mut new_lines: Vec<String> = Vec::new();
        let mut in_servers: bool = false;

        for line in lines {
            if line.contains("[SERVERS.") {
                in_servers = true;
                // new_lines.push("# ".to_string());
                new_lines.push(line);
            } else if in_servers && line.starts_with('[') && !line.contains("SERVERS") {
                in_servers = false;
                new_lines.push(line);
            } else {
                new_lines.push(line);
            }
        }

        result = new_lines.join("\n");

        result
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        if !self.servers.contains_key(&self.default_region) {
            anyhow::bail!(
                "default_region '{}' not found in servers list. Available: {}",
                self.default_region,
                self.servers.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        // 验证目录路径是否有效
        let dirs: [&String; 5] = [
            &self.file_path.root_output_dir,
            &self.file_path.manifest_output_dir,
            &self.file_path.versions_output_dir,
            &self.file_path.patch_output_dir,
            &self.file_path.unpack_output_dir,
        ];

        for dir in dirs {
            if dir.is_empty() {
                anyhow::bail!("Directory path cannot be empty: {}", dir);
            }
        }

        // 验证文件名是否为空
        let names: [&String; 5] = [
            &self.file_name.manifest_raw_file,
            &self.file_name.manifest_decrypt_file,
            &self.file_name.manifest_decoded_file,
            &self.file_name.patch_manifest_file,
            &self.file_name.version_file,
        ];

        for name in names {
            if name.is_empty() {
                anyhow::bail!("File name cannot be empty");
            }
        }

        // 验证超时时间
        if self.request.timeout_secs == 0 {
            anyhow::bail!("TIMEOUT_SECS must be greater than 0");
        }

        // 如果启用默认区域模式，检查默认区域是否存在
        if self.feature_flags.is_use_default_region {
            if !self.servers.contains_key(&self.default_region) {
                anyhow::bail!(
                    "default_region '{}' not found in servers list when is_use_default_region is true",
                    self.default_region
                );
            }
        }

        // 校验资源正则
        if self.feature_flags.is_use_resource_regex {
            for pattern in &self.resource_registry.base_resource_patterns {
                regex::Regex::new(pattern)
                    .with_context(|| format!("Invalid resource regex pattern: '{}'", pattern))?;
            }
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut servers: IndexMap<String, ServerConfigOwned> = IndexMap::new();
        for &region in SERVER_ORDER {
            if let Some(cfg) = STATIC_RESOURCE_SERVERS.get(region) {
                servers.insert(region.to_string(), ServerConfigOwned::from(cfg));
            }
        }

        Self {
            default_region: "CN".into(),
            server_route: ServerRouteConfigs::default(),
            file_path: FilePathConfigs::default(),
            file_name: FileNameConfigs::default(),
            extractor: ExtractorConfig::default(),
            request: RequestConfigs::default(),
            resource_registry: ResourceRegistryConfig::default(),
            servers,
            feature_flags: FeatureFlags::default(),
        }
    }
}

// ============================================================
// 加载 AppConfig 相关的辅助函数
// ============================================================
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

    if cfg.feature_flags.is_use_default_region {
        let default_region: String = cfg.default_region.clone();
        cfg.servers.retain(|region, _| region == &default_region);
        logger::tips!(
            "IS_USE_DEFAULT_REGION is enabled, only '{}' region is kept !!!",
            cfg.default_region
        );
    }

    cfg.validate()?;

    Ok(cfg)
}

/// 全局配置实例
pub static APP_CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| load_or_init_config(CONFIG_FILE_NAME).expect("failed to load config"));

// ============================================================
// 向后兼容类型定义
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub struct ManifestEntry {
    #[serde(rename = "FileName")]
    pub file_name: String,
    #[serde(rename = "Hash")]
    pub hash: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "AdditionalPath")]
    pub additional_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub struct ResourceEntry {
    pub file_name: String,
    pub hash: String,
    pub version: u64,
    pub additional_path: String,
}

impl From<&FileDiff> for ResourceEntry {
    fn from(fd: &FileDiff) -> Self {
        ResourceEntry {
            file_name: fd.file_name.clone(),
            hash: fd.hash.clone(),
            version: fd.version as u64,
            additional_path: fd.additional_path.clone(),
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config: AppConfig = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config: AppConfig = AppConfig::default();
        let toml_str: String = config.to_toml_with_comments();
        // 验证可以反序列化回来
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.default_region, config.default_region);
        assert_eq!(parsed.servers.len(), config.servers.len());
    }

    #[test]
    fn test_config_generation() {
        let temp_dir: std::path::PathBuf = std::env::temp_dir().join("config_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_path: std::path::PathBuf = temp_dir.join("config.toml");

        // 确保文件不存在
        let _ = std::fs::remove_file(&config_path);

        // 加载配置（应该生成）
        let _: AppConfig = load_or_init_config(&config_path).unwrap();

        // 验证文件已创建
        assert!(config_path.exists(), "Config file was not created!");

        // 验证内容可以读取
        let content: String = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.is_empty(), "Config file is empty!");

        logger::test!(
            "Config generated successfully at: {}",
            config_path.display()
        );
        log::info!("Content:\n{}", content);

        // 清理
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
