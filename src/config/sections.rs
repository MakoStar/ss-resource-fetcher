use std::collections::HashMap;
use std::time::Duration;

use network_manager::FetcherConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerRouteConfig {
    /// 清单文件的 API 路径
    #[serde(rename = "MANIFEST_ROUTE")]
    pub manifest_route: String,

    /// 资源文件的 API 路径
    #[serde(rename = "RESOURCE_ROUTE")]
    pub resource_route: String,
}

impl Default for ServerRouteConfig {
    fn default() -> Self {
        Self {
            manifest_route: "/meta/win.html".into(),
            resource_route: "/res/win/".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器地址
    #[serde(rename = "URL")]
    pub url: String,

    /// 解密密钥
    #[serde(rename = "KEY")]
    pub key: String,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticServer {
    /// 服务器地址
    pub url: &'static str,
    /// 解密密钥
    pub key: &'static str,
}

pub(crate) static STATIC_RESOURCE_SERVERS: phf::OrderedMap<&'static str, StaticServer> = phf::phf_ordered_map! {
    "CN" => StaticServer { url: "https://nova-static.yostar.cn", key: "Xf&FRcsYm48cJ2A@" },
    "EN" => StaticServer { url: "https://nova-static.stellasora.global", key: "ma5Dn2FhC*Xhxy%c" },
    "JP" => StaticServer { url: "https://nova-static.stellasora.jp", key: "ZnUFA@S9%4KyoryM" },
    "KR" => StaticServer { url: "https://nova-static.stellasora.kr", key: "U9cjHuwGDDx&$drn" },
    "TW" => StaticServer { url: "https://nova-static.stargazer-games.com", key: "owGYVDmfHrxi^4pm" },
};

impl From<&StaticServer> for ServerConfig {
    fn from(s: &StaticServer) -> Self {
        Self {
            url: s.url.into(),
            key: s.key.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorConfig {
    /// Uncensor 清单所在服务器
    #[serde(rename = "URL")]
    pub url: String,

    /// 清单在该服务器上的路径
    #[serde(rename = "MANIFEST_ROUTE")]
    pub manifest_route: String,

    /// 实际下载资源时使用的官方区域
    #[serde(rename = "SOURCE_REGION")]
    pub source_region: String,
}

impl Default for UncensorConfig {
    fn default() -> Self {
        Self {
            url: "http://na.jvav.net.cn".into(),
            manifest_route: "res/win/ss_win.mani".into(),
            source_region: "TW".into(),
        }
    }
}

impl UncensorConfig {
    pub fn manifest_url(&self) -> String {
        format!(
            "{}/{}",
            self.url.trim_end_matches('/'),
            self.manifest_route.trim_start_matches('/'),
        )
    }

    pub fn resource_base_url(&self) -> String {
        let base = self.url.trim_end_matches('/');
        let route = self.manifest_route.trim_start_matches('/');

        match route.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => format!("{base}/{dir}"),
            _ => base.to_string(),
        }
    }

    pub fn manifest_file_name(&self) -> &str {
        self.manifest_route
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("ss_win.mani")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilePathConfig {
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

    /// 反和谐资源输出目录
    #[serde(rename = "UNCENSOR_OUTPUT_DIR")]
    pub uncensor_output_dir: String,

    /// 反和谐默认地址资源输出目录
    #[serde(rename = "UNCENSOR_DEFAULT_OUTPUT_DIR")]
    pub uncensor_default_output_dir: String,
}

impl Default for FilePathConfig {
    fn default() -> Self {
        Self {
            root_output_dir: "./output/".into(),
            manifest_output_dir: "./output/Manifest".into(),
            versions_output_dir: "./output/Manifest".into(),
            patch_output_dir: "./output/Metadata".into(),
            unpack_output_dir: "./output/Unpack".into(),
            uncensor_output_dir: "./output/Uncensor".into(),
            uncensor_default_output_dir: "./output/UncensorDefault".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileNameConfig {
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

impl Default for FileNameConfig {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RequestConfig {
    /// 是否验证 TLS 证书
    #[serde(rename = "VERIFY_TLS")]
    pub verify_tls: bool,

    /// 请求超时时间（秒）
    #[serde(rename = "TIMEOUT_SECS")]
    pub timeout_secs: u64,

    /// 总重试次数
    #[serde(rename = "TOTAL_RETRIES")]
    pub total_retries: u32,

    /// 退避基准间隔（毫秒）
    #[serde(rename = "BACKOFF_BASE_MS")]
    pub backoff_base_ms: u32,

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

impl RequestConfig {
    pub fn build_fetcher_config(&self) -> FetcherConfig {
        FetcherConfig::builder()
            .timeout(self.timeout_secs)
            .max_retries(self.total_retries)
            .backoff_base_ms(self.backoff_base_ms)
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

impl Default for RequestConfig {
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
            backoff_base_ms: 1000,
            pool_max_idle: 50,
            min_retry_delay_ms: 500,
            max_retry_delay_secs: 30,
            extra_headers: headers,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::UncensorConfig;

    #[test]
    fn builds_default_uncensor_urls() {
        let config = UncensorConfig::default();

        assert_eq!(
            config.manifest_url(),
            "http://na.jvav.net.cn/res/win/ss_win.mani"
        );
        assert_eq!(config.resource_base_url(), "http://na.jvav.net.cn/res/win");
    }

    #[test]
    fn resource_base_url_points_at_manifest_dir() {
        let config = UncensorConfig {
            url: "https://example.com/".into(),
            manifest_route: "/a/b/c/other.mani".into(),
            source_region: "TW".into(),
        };

        assert_eq!(
            config.manifest_url(),
            "https://example.com/a/b/c/other.mani"
        );
        assert_eq!(config.resource_base_url(), "https://example.com/a/b/c");
    }

    #[test]
    fn resource_base_url_falls_back_to_root() {
        let config = UncensorConfig {
            url: "https://example.com".into(),
            manifest_route: "ss_win.mani".into(),
            source_region: "TW".into(),
        };

        assert_eq!(config.resource_base_url(), "https://example.com");
    }

    #[test]
    fn manifest_file_name_takes_last_route_segment() {
        let config = UncensorConfig::default();
        assert_eq!(config.manifest_file_name(), "ss_win.mani");

        let config = UncensorConfig {
            manifest_route: "/res/win/other.mani".into(),
            ..Default::default()
        };
        assert_eq!(config.manifest_file_name(), "other.mani");

        let config = UncensorConfig {
            manifest_route: String::new(),
            ..Default::default()
        };
        assert_eq!(config.manifest_file_name(), "ss_win.mani");
    }
}
