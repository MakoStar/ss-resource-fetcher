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
