use std::collections::HashMap;
use std::time::Duration;

use crate::body::RequestBody;
use crate::method::HttpMethod;

pub struct RequestSpec {
    pub method: HttpMethod,
    pub url: String,
    pub query_params: Vec<(String, String)>,
    pub headers: HashMap<String, String>,
    pub body: RequestBody,
    pub timeout_override: Option<Duration>,
    pub require_success: bool,
}

impl RequestSpec {
    pub fn builder(method: HttpMethod, url: impl Into<String>) -> RequestSpecBuilder {
        RequestSpecBuilder(Self {
            method,
            url: url.into(),
            query_params: Vec::new(),
            headers: HashMap::new(),
            body: RequestBody::None,
            timeout_override: None,
            require_success: true,
        })
    }
}

pub struct RequestSpecBuilder(RequestSpec);

impl RequestSpecBuilder {
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.query_params.push((key.into(), value.into()));
        self
    }

    pub fn queries(mut self, params: Vec<(String, String)>) -> Self {
        self.0.query_params.extend(params);
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.headers.insert(key.into(), value.into());
        self
    }

    pub fn headers(mut self, map: HashMap<String, String>) -> Self {
        self.0.headers.extend(map);
        self
    }

    pub fn bearer_token(mut self, token: impl Into<String>) -> Self {
        self.0
            .headers
            .insert("Authorization".into(), format!("Bearer {}", token.into()));
        self
    }

    pub fn content_type(mut self, ct: impl Into<String>) -> Self {
        self.0.headers.insert("Content-Type".into(), ct.into());
        self
    }

    pub fn body(mut self, body: RequestBody) -> Self {
        self.0.body = body;
        self
    }

    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Self {
        if let Ok(body) = RequestBody::json(value) {
            self.0.body = body;
        }
        self
    }

    pub fn form(mut self, pairs: Vec<(String, String)>) -> Self {
        self.0.body = RequestBody::Form(pairs);
        self
    }

    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.0.body = RequestBody::Text(s.into());
        self
    }

    pub fn bytes(mut self, b: Vec<u8>) -> Self {
        self.0.body = RequestBody::Bytes(b);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.0.timeout_override = Some(d);
        self
    }

    pub fn allow_any_status(mut self) -> Self {
        self.0.require_success = false;
        self
    }

    pub fn build(self) -> RequestSpec {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::{HttpMethod, RequestBody, RequestSpec};

    #[test]
    fn builder_defaults_are_sane() {
        let spec = RequestSpec::builder(HttpMethod::Get, "https://example.com/res").build();

        assert_eq!(spec.method, HttpMethod::Get);
        assert_eq!(spec.url, "https://example.com/res");
        assert!(spec.query_params.is_empty());
        assert!(spec.headers.is_empty());
        assert!(matches!(spec.body, RequestBody::None));
        assert_eq!(spec.timeout_override, None);
        assert!(spec.require_success);
    }

    #[test]
    fn query_params_accumulate_in_order() {
        let spec = RequestSpec::builder(HttpMethod::Get, "https://example.com")
            .query("b", "2")
            .queries(vec![("a".to_string(), "1".to_string())])
            .query("c", "3")
            .build();

        assert_eq!(
            spec.query_params,
            vec![
                ("b".to_string(), "2".to_string()),
                ("a".to_string(), "1".to_string()),
                ("c".to_string(), "3".to_string()),
            ]
        );
    }

    #[test]
    fn header_helpers_set_expected_names() {
        let spec = RequestSpec::builder(HttpMethod::Post, "https://example.com")
            .header("X-Custom", "v")
            .bearer_token("tok")
            .content_type("application/json")
            .build();

        assert_eq!(spec.headers.get("X-Custom").map(String::as_str), Some("v"));
        assert_eq!(
            spec.headers.get("Authorization").map(String::as_str),
            Some("Bearer tok")
        );
        assert_eq!(
            spec.headers.get("Content-Type").map(String::as_str),
            Some("application/json")
        );
    }

    #[test]
    fn headers_map_merges() {
        let mut map = HashMap::new();
        map.insert("A".to_string(), "1".to_string());
        map.insert("B".to_string(), "2".to_string());

        let spec = RequestSpec::builder(HttpMethod::Get, "https://example.com")
            .header("C", "3")
            .headers(map)
            .build();

        assert_eq!(spec.headers.len(), 3);
        assert_eq!(spec.headers.get("B").map(String::as_str), Some("2"));
    }

    #[test]
    fn body_setters_cover_every_variant() {
        let json = RequestSpec::builder(HttpMethod::Post, "https://example.com")
            .json(&serde_json::json!({"k": "v"}))
            .build();
        assert!(matches!(json.body, RequestBody::Json(_)));

        let form = RequestSpec::builder(HttpMethod::Post, "https://example.com")
            .form(vec![("f".to_string(), "1".to_string())])
            .build();
        assert!(matches!(form.body, RequestBody::Form(pairs) if pairs.len() == 1));

        let text = RequestSpec::builder(HttpMethod::Post, "https://example.com")
            .text("hello")
            .build();
        assert!(matches!(&text.body, RequestBody::Text(s) if s == "hello"));

        let bytes = RequestSpec::builder(HttpMethod::Post, "https://example.com")
            .bytes(vec![1, 2, 3])
            .build();
        assert!(matches!(&bytes.body, RequestBody::Bytes(b) if b.len() == 3));

        let none = RequestSpec::builder(HttpMethod::Get, "https://example.com")
            .body(RequestBody::None)
            .build();
        assert!(matches!(none.body, RequestBody::None));
    }

    #[test]
    fn allow_any_status_flips_requirement() {
        let strict = RequestSpec::builder(HttpMethod::Get, "https://example.com").build();
        let loose = RequestSpec::builder(HttpMethod::Get, "https://example.com")
            .allow_any_status()
            .build();

        assert!(strict.require_success);
        assert!(!loose.require_success);
    }

    #[test]
    fn timeout_override_is_optional() {
        let spec = RequestSpec::builder(HttpMethod::Get, "https://example.com")
            .timeout(Duration::from_millis(250))
            .build();

        assert_eq!(spec.timeout_override, Some(Duration::from_millis(250)));
    }
}
