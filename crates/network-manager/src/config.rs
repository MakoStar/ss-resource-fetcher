use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FetcherConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub backoff_base_ms: u32,
    pub retry_min_delay: Duration,
    pub retry_max_delay: Duration,
    pub verify_tls: bool,
    pub default_headers: HashMap<String, String>,
    pub pool_max_idle: usize,
    pub redirect_follow: bool,
    pub redirect_max: usize,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            backoff_base_ms: 1000,
            retry_min_delay: Duration::from_millis(500),
            retry_max_delay: Duration::from_secs(15),
            verify_tls: false,
            default_headers: HashMap::new(),
            pool_max_idle: 10,
            redirect_follow: true,
            redirect_max: 10,
        }
    }
}

impl FetcherConfig {
    pub fn builder() -> FetcherConfigBuilder {
        FetcherConfigBuilder(Self::default())
    }
}

pub struct FetcherConfigBuilder(FetcherConfig);

impl FetcherConfigBuilder {
    pub fn timeout(mut self, secs: u64) -> Self {
        self.0.timeout = Duration::from_secs(secs);
        self
    }

    pub fn timeout_duration(mut self, d: Duration) -> Self {
        self.0.timeout = d;
        self
    }

    pub fn max_retries(mut self, n: u32) -> Self {
        self.0.max_retries = n;
        self
    }

    pub fn backoff_base_ms(mut self, ms: u32) -> Self {
        self.0.backoff_base_ms = ms;
        self
    }

    pub fn retry_bounds(mut self, min: Duration, max: Duration) -> Self {
        self.0.retry_min_delay = min;
        self.0.retry_max_delay = max;
        self
    }

    pub fn verify_tls(mut self, v: bool) -> Self {
        self.0.verify_tls = v;
        self
    }

    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.0.default_headers.insert(k.into(), v.into());
        self
    }

    pub fn headers(mut self, map: HashMap<String, String>) -> Self {
        self.0.default_headers.extend(map);
        self
    }

    pub fn pool_max_idle(mut self, n: usize) -> Self {
        self.0.pool_max_idle = n;
        self
    }

    pub fn redirect(mut self, follow: bool, max: usize) -> Self {
        self.0.redirect_follow = follow;
        self.0.redirect_max = max;
        self
    }

    pub fn build(self) -> FetcherConfig {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn defaults_match_documented_values() {
        let config = FetcherConfig::default();

        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.backoff_base_ms, 1000);
        assert_eq!(config.retry_min_delay, Duration::from_millis(500));
        assert_eq!(config.retry_max_delay, Duration::from_secs(15));
        assert!(!config.verify_tls);
        assert!(config.default_headers.is_empty());
        assert_eq!(config.pool_max_idle, 10);
        assert!(config.redirect_follow);
        assert_eq!(config.redirect_max, 10);
    }

    #[test]
    fn builder_overrides_every_field() {
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), "probe".to_string());

        let config = FetcherConfig::builder()
            .timeout(90)
            .timeout_duration(Duration::from_millis(1500))
            .max_retries(7)
            .backoff_base_ms(250)
            .retry_bounds(Duration::from_millis(100), Duration::from_secs(5))
            .verify_tls(true)
            .header("X-Probe", "1")
            .headers(headers)
            .pool_max_idle(42)
            .redirect(false, 3)
            .build();

        assert_eq!(config.timeout, Duration::from_millis(1500));
        assert_eq!(config.max_retries, 7);
        assert_eq!(config.backoff_base_ms, 250);
        assert_eq!(config.retry_min_delay, Duration::from_millis(100));
        assert_eq!(config.retry_max_delay, Duration::from_secs(5));
        assert!(config.verify_tls);
        assert_eq!(
            config.default_headers.get("X-Probe").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            config.default_headers.get("User-Agent").map(String::as_str),
            Some("probe")
        );
        assert_eq!(config.pool_max_idle, 42);
        assert!(!config.redirect_follow);
        assert_eq!(config.redirect_max, 3);
    }

    #[test]
    fn header_calls_accumulate() {
        let config = FetcherConfig::builder()
            .header("A", "1")
            .header("B", "2")
            .header("A", "3")
            .build();

        assert_eq!(config.default_headers.len(), 2);
        assert_eq!(
            config.default_headers.get("A").map(String::as_str),
            Some("3")
        );
    }
}
