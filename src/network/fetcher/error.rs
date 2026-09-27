use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("unknown region: {region}")]
    UnknownRegion { region: String },

    #[error(transparent)]
    Fetch(#[from] network_manager::FetchError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

#[derive(Error, Debug)]
pub enum ResourceError {
    #[error(transparent)]
    Fetch(#[from] network_manager::FetchError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type TManifestResult<T> = Result<T, ManifestError>;
pub type TResourceResult<T> = Result<T, ResourceError>;
