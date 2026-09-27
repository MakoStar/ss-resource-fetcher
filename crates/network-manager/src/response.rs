use serde::de::DeserializeOwned;
use crate::error::{FetchError, FetchResult};

pub struct FetchResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
}

impl FetchResponse {
    pub fn as_bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn as_text(&self) -> FetchResult<String> {
        String::from_utf8(self.body.clone())
            .map_err(|e| FetchError::Utf8Decode {
                url: String::new(),
                source: e,
            })
    }

    pub fn as_json<T: DeserializeOwned>(&self) -> FetchResult<T> {
        serde_json::from_slice(&self.body)
            .map_err(|e| FetchError::JsonParse {
                url: String::new(),
                source: e,
            })
    }

    pub fn as_json_value(&self) -> FetchResult<serde_json::Value> {
        self.as_json()
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}
