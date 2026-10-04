use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractorConfig {
    /// 默认版本键值
    #[serde(rename = "DEFAULT_VERSION_KEY")]
    pub default_version_key: String,

    /// 客户端版本键
    #[serde(rename = "CLIENT_VERSION_KEY")]
    pub client_version_key: String,

    /// 游戏版本键
    #[serde(rename = "GAME_VERSION_KEY")]
    pub game_version_key: String,

    /// 补丁版本键
    #[serde(rename = "PATCH_VERSION_KEY")]
    pub patch_version_key: String,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            default_version_key: "unknown".into(),
            client_version_key: "CLIENT_VER".into(),
            game_version_key: "GAME_VER".into(),
            patch_version_key: "BIN_DIFF_PATCH_VER".into(),
        }
    }
}
