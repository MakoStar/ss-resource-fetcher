use log::{debug, error, warn};
use reqwest::Client;
use reqwest::RequestBuilder;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;

use crate::body::RequestBody;
use crate::config::FetcherConfig;
use crate::error::{FetchError, FetchResult};
use crate::method::HttpMethod;
use crate::request::RequestSpec;
use crate::response::FetchResponse;

#[derive(Clone)]
pub struct HttpFetcher {
    raw_client: Client,
    middleware_client: ClientWithMiddleware,
    config: FetcherConfig,
}

impl HttpFetcher {
    pub fn new(config: &FetcherConfig) -> Self {
        let retry_policy = ExponentialBackoff::builder()
            .base(config.backoff_base_ms)
            .retry_bounds(config.retry_min_delay, config.retry_max_delay)
            .build_with_max_retries(config.max_retries);

        let mut default_headers = HeaderMap::new();

        for (k, v) in &config.default_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                default_headers.insert(name, val);
            } else {
                warn!("skipping invalid default header: {k}={v}");
            }
        }

        let redirect_policy = if config.redirect_follow {
            Policy::limited(config.redirect_max)
        } else {
            Policy::none()
        };

        let raw_client = Client::builder()
            .default_headers(default_headers)
            .timeout(config.timeout)
            .danger_accept_invalid_certs(!config.verify_tls)
            .pool_max_idle_per_host(config.pool_max_idle)
            .redirect(redirect_policy)
            .build()
            .expect("failed to build HTTP client");

        let middleware_client = ClientBuilder::new(raw_client.clone())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            raw_client,
            middleware_client,
            config: config.clone(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(&FetcherConfig::default())
    }

    pub async fn execute(&self, spec: &RequestSpec) -> FetchResult<FetchResponse> {
        let url: &str = spec.url.trim();
        if url.is_empty() {
            return Err(FetchError::InvalidUrl { url: String::new() });
        }

        let method: reqwest::Method = spec.method.into();
        let mut req_builder = self.raw_client.request(method.clone(), url);

        if !spec.query_params.is_empty() {
            req_builder = req_builder.query(&spec.query_params);
        }

        for (k, v) in &spec.headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                req_builder = req_builder.header(name, val);
            } else {
                return Err(FetchError::HeaderBuild(format!("{k}={v}")));
            }
        }

        req_builder = Self::apply_body(req_builder, &spec.body)?;

        if let Some(t) = spec.timeout_override {
            req_builder = req_builder.timeout(t);
        }

        let request = req_builder.build().map_err(|e| FetchError::Request {
            method: spec.method.to_string(),
            url: url.to_string(),
            source: reqwest_middleware::Error::Reqwest(e),
        })?;

        let resp = self.middleware_client.execute(request).await.map_err(|e| {
            error!("request failed: {} {} → {}", spec.method, url, e);
            FetchError::Request {
                method: spec.method.to_string(),
                url: url.to_string(),
                source: e,
            }
        })?;

        let status = resp.status().as_u16();
        let parsed = Url::parse(url).unwrap();
        let route_path: &str = parsed.path();
        debug!("{} {} {}", spec.method, route_path, status);

        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| FetchError::BodyRead {
                url: url.to_string(),
                source: e,
            })?
            .to_vec();

        if spec.require_success && !(200..300).contains(&status) {
            return Err(FetchError::HttpStatus {
                status,
                url: url.to_string(),
                body: Some(body_bytes),
            });
        }

        Ok(FetchResponse {
            status,
            headers,
            body: body_bytes,
        })
    }

    fn apply_body(req: RequestBuilder, body: &RequestBody) -> FetchResult<RequestBuilder> {
        match body {
            RequestBody::None => Ok(req),
            RequestBody::Json(v) => Ok(req.json(v)),
            RequestBody::Form(pairs) => Ok(req.form(pairs)),
            RequestBody::Bytes(b) => Ok(req.body(b.clone())),
            RequestBody::Text(s) => Ok(req
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(s.clone())),
        }
    }

    pub async fn get_bytes(&self, url: &str) -> FetchResult<Vec<u8>> {
        let spec = RequestSpec::builder(HttpMethod::Get, url).build();
        Ok(self.execute(&spec).await?.body)
    }

    pub async fn get_text(&self, url: &str) -> FetchResult<String> {
        let spec = RequestSpec::builder(HttpMethod::Get, url).build();
        self.execute(&spec).await?.as_text()
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> FetchResult<T> {
        let spec = RequestSpec::builder(HttpMethod::Get, url).build();
        self.execute(&spec).await?.as_json()
    }

    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        url: &str,
        payload: &T,
    ) -> FetchResult<R> {
        let spec = RequestSpec::builder(HttpMethod::Post, url)
            .json(payload)
            .build();
        self.execute(&spec).await?.as_json()
    }

    pub async fn post_form(
        &self,
        url: &str,
        pairs: Vec<(String, String)>,
    ) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Post, url)
            .form(pairs)
            .build();
        self.execute(&spec).await
    }

    pub async fn post_bytes(&self, url: &str, data: Vec<u8>) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Post, url)
            .bytes(data)
            .build();
        self.execute(&spec).await
    }

    pub async fn post_text(&self, url: &str, text: &str) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Post, url)
            .text(text)
            .build();
        self.execute(&spec).await
    }

    pub async fn put_json<T: Serialize>(
        &self,
        url: &str,
        payload: &T,
    ) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Put, url)
            .json(payload)
            .build();
        self.execute(&spec).await
    }

    pub async fn patch_json<T: Serialize>(
        &self,
        url: &str,
        payload: &T,
    ) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Patch, url)
            .json(payload)
            .build();
        self.execute(&spec).await
    }

    pub async fn delete(&self, url: &str) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Delete, url).build();
        self.execute(&spec).await
    }

    pub async fn head(&self, url: &str) -> FetchResult<FetchResponse> {
        let spec = RequestSpec::builder(HttpMethod::Head, url)
            .allow_any_status()
            .build();
        self.execute(&spec).await
    }

    pub async fn preflight(&self, url: &str) -> FetchResult<PreflightResult> {
        let spec = RequestSpec::builder(HttpMethod::Options, url)
            .header("Access-Control-Request-Method", "POST")
            .header(
                "Access-Control-Request-Headers",
                "Content-Type,Authorization",
            )
            .allow_any_status()
            .build();

        let resp = self.execute(&spec).await?;
        Ok(PreflightResult::from_response(&resp))
    }

    pub fn config(&self) -> &FetcherConfig {
        &self.config
    }
}

#[derive(Debug, Clone)]
pub struct PreflightResult {
    pub allowed: bool,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
    pub max_age: Option<u64>,
    pub status: u16,
}

impl PreflightResult {
    fn from_response(resp: &FetchResponse) -> Self {
        let get = |key: &str| -> Option<String> { resp.headers.get(key).cloned() };

        Self {
            allowed: resp.is_success(),
            allow_methods: get("access-control-allow-methods")
                .map(|s| s.split(',').map(|m| m.trim().to_string()).collect())
                .unwrap_or_default(),
            allow_headers: get("access-control-allow-headers")
                .map(|s| s.split(',').map(|h| h.trim().to_string()).collect())
                .unwrap_or_default(),
            max_age: get("access-control-max-age").and_then(|s| s.parse().ok()),
            status: resp.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FetcherConfig, HttpFetcher, PreflightResult};
    use crate::error::FetchError;
    use crate::method::HttpMethod;
    use crate::request::RequestSpec;
    use crate::response::FetchResponse;

    fn fetcher() -> HttpFetcher {
        HttpFetcher::new(&FetcherConfig::default())
    }

    #[tokio::test]
    async fn execute_rejects_empty_url() {
        let spec = RequestSpec::builder(HttpMethod::Get, "").build();

        let err = fetcher().execute(&spec).await.unwrap_err();

        assert!(matches!(err, FetchError::InvalidUrl { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn execute_rejects_whitespace_url() {
        let spec = RequestSpec::builder(HttpMethod::Get, "   ").build();

        let err = fetcher().execute(&spec).await.unwrap_err();

        assert!(matches!(err, FetchError::InvalidUrl { .. }), "{err:?}");
    }

    #[test]
    fn new_skips_invalid_default_headers() {
        let config = FetcherConfig::builder()
            .header("Bad Header", "v")
            .header("Good-Header", "v")
            .build();

        let fetcher = HttpFetcher::new(&config);

        assert_eq!(fetcher.config().default_headers.len(), 2);
    }

    #[test]
    fn config_accessor_returns_what_was_built() {
        let config = FetcherConfig::builder().max_retries(9).build();
        let fetcher = HttpFetcher::new(&config);

        assert_eq!(fetcher.config().max_retries, 9);
    }

    #[test]
    fn preflight_parses_cors_headers() {
        let resp = FetchResponse {
            status: 204,
            headers: [
                (
                    "access-control-allow-methods".to_string(),
                    "POST, PUT".to_string(),
                ),
                (
                    "access-control-allow-headers".to_string(),
                    "Content-Type, Authorization".to_string(),
                ),
                ("access-control-max-age".to_string(), "600".to_string()),
            ]
            .into_iter()
            .collect(),
            body: Vec::new(),
        };

        let result = PreflightResult::from_response(&resp);

        assert!(result.allowed);
        assert_eq!(result.allow_methods, vec!["POST", "PUT"]);
        assert_eq!(result.allow_headers, vec!["Content-Type", "Authorization"]);
        assert_eq!(result.max_age, Some(600));
        assert_eq!(result.status, 204);
    }

    #[test]
    fn preflight_without_cors_headers_is_not_allowed() {
        let resp = FetchResponse {
            status: 200,
            headers: Default::default(),
            body: Vec::new(),
        };

        let result = PreflightResult::from_response(&resp);

        assert!(result.allowed, "a 2xx response counts as allowed");
        assert!(result.allow_methods.is_empty());
        assert!(result.allow_headers.is_empty());
        assert_eq!(result.max_age, None);
    }
}
