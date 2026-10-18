use thiserror::Error;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("empty or invalid URL: {url}")]
    InvalidUrl { url: String },

    #[error("request failed: {method} {url} → {source}")]
    Request {
        method: String,
        url: String,
        #[source]
        source: reqwest_middleware::Error,
    },

    #[error("read body failed: {url} → {source}")]
    BodyRead {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("JSON serialize failed: {source}")]
    JsonSerialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("JSON parse failed: {url} → {source}")]
    JsonParse {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("UTF-8 decode failed: {url} → {source}")]
    Utf8Decode {
        url: String,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("unexpected status {status}: {url}")]
    HttpStatus {
        status: u16,
        url: String,
        body: Option<Vec<u8>>,
    },

    #[error("header build error: {0}")]
    HeaderBuild(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type FetchResult<T> = Result<T, FetchError>;

#[cfg(test)]
mod tests {
    use super::FetchError;

    fn invalid_utf8_error() -> FetchError {
        FetchError::Utf8Decode {
            url: "https://example.com/res".into(),
            source: String::from_utf8(vec![0xff]).unwrap_err(),
        }
    }

    #[test]
    fn invalid_url_message_names_the_url() {
        let err = FetchError::InvalidUrl { url: "".into() };

        assert_eq!(err.to_string(), "empty or invalid URL: ");
    }

    #[test]
    fn http_status_message_carries_code_and_url() {
        let err = FetchError::HttpStatus {
            status: 404,
            url: "https://example.com/missing".into(),
            body: None,
        };

        assert_eq!(
            err.to_string(),
            "unexpected status 404: https://example.com/missing"
        );
    }

    #[test]
    fn utf8_error_message_carries_url() {
        let err = invalid_utf8_error();

        assert!(err.to_string().contains("UTF-8 decode failed"), "{err}");
        assert!(err.to_string().contains("https://example.com/res"), "{err}");
    }

    #[test]
    fn io_error_converts_from_std() {
        let err: FetchError = std::io::Error::other("disk gone").into();

        assert!(err.to_string().contains("disk gone"), "{err}");
    }
}
