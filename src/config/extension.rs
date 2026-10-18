use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtensionConfig {
    /// 清单扩展名
    #[serde(rename = "MANI")]
    pub mani: String,
}

impl Default for ExtensionConfig {
    fn default() -> Self {
        Self {
            mani: ".mani".into(),
        }
    }
}
