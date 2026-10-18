use crate::error::{FetchError, FetchResult};
use serde::de::DeserializeOwned;

#[derive(Debug)]
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
        String::from_utf8(self.body.clone()).map_err(|e| FetchError::Utf8Decode {
            url: String::new(),
            source: e,
        })
    }

    pub fn as_json<T: DeserializeOwned>(&self) -> FetchResult<T> {
        serde_json::from_slice(&self.body).map_err(|e| FetchError::JsonParse {
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

#[cfg(test)]
mod tests {
    use super::FetchResponse;
    use crate::error::FetchError;

    fn response(status: u16, body: &[u8]) -> FetchResponse {
        FetchResponse {
            status,
            headers: Default::default(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn as_bytes_returns_body() {
        assert_eq!(response(200, b"abc").as_bytes(), b"abc");
    }

    #[test]
    fn as_text_decodes_utf8() {
        assert_eq!(response(200, "你好".as_bytes()).as_text().unwrap(), "你好");
    }

    #[test]
    fn as_text_reports_invalid_utf8() {
        let err = response(200, &[0xff, 0xfe]).as_text().unwrap_err();

        assert!(matches!(err, FetchError::Utf8Decode { .. }), "{err:?}");
    }

    #[test]
    fn as_json_parses_body() {
        let resp = response(200, br#"{"name":"stella","count":7}"#);
        let value: serde_json::Value = resp.as_json().unwrap();

        assert_eq!(value["name"], "stella");
        assert_eq!(value["count"], 7);
    }

    #[test]
    fn as_json_reports_parse_failure() {
        let err = response(200, b"not json")
            .as_json::<serde_json::Value>()
            .unwrap_err();

        assert!(matches!(err, FetchError::JsonParse { .. }), "{err:?}");
    }

    #[test]
    fn as_json_value_matches_as_json() {
        let resp = response(200, br#"{"a":1}"#);

        assert_eq!(
            resp.as_json_value().unwrap(),
            resp.as_json::<serde_json::Value>().unwrap()
        );
    }

    #[test]
    fn is_success_covers_2xx_only() {
        assert!(response(200, b"").is_success());
        assert!(response(204, b"").is_success());
        assert!(response(299, b"").is_success());

        assert!(!response(199, b"").is_success());
        assert!(!response(300, b"").is_success());
        assert!(!response(404, b"").is_success());
        assert!(!response(500, b"").is_success());
    }
}
