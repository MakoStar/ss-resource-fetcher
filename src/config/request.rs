use std::collections::HashMap;
use std::time::Duration;

use network_manager::FetcherConfig;
use serde::{Deserialize, Serialize};

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
