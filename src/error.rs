use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    /// 未知区域
    #[error("unknown region: {0}")]
    UnknownRegion(String),

    /// 区域缺少服务器配置
    #[error("region '{0}' has no server config")]
    MissingServer(String),

    /// 区域缺少解密密钥
    #[error("region '{0}' has no decryption key")]
    MissingKey(String),

    /// 数据为空
    #[error("empty data: {context}")]
    EmptyData {
        /// 上下文说明
        context: String,
    },

    /// 补丁清单版本解析失败
    #[error("failed to resolve patch manifest version: {0}")]
    PatchVersion(String),

    /// 网络请求失败
    #[error(transparent)]
    Fetch(#[from] network_manager::FetchError),

    /// 文件读写失败
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// 序列化失败
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// 其他错误
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Anyhow(anyhow::anyhow!(msg.into()))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
