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
