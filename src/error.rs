use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("unknown region: {0}")]
    UnknownRegion(String),

    #[error("region '{0}' has no server config")]
    MissingServer(String),

    #[error("region '{0}' has no decryption key")]
    MissingKey(String),

    #[error("empty data: {context}")]
    EmptyData { context: String },

    #[error("failed to resolve patch manifest version: {0}")]
    PatchVersion(String),

    #[error(transparent)]
    Fetch(#[from] network_manager::FetchError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Anyhow(anyhow::anyhow!(msg.into()))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
