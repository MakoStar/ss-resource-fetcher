use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateTagConfig {
    /// 补丁标签前缀
    #[serde(rename = "PATCH_PREFIX")]
    pub patch_prefix: String,

    /// 增量补丁后缀
    #[serde(rename = "U_SUFFIX")]
    pub u_suffix: String,

    /// 整文件补丁后缀
    #[serde(rename = "N_SUFFIX")]
    pub n_suffix: String,

    /// 清单补丁后缀
    #[serde(rename = "M_SUFFIX")]
    pub m_suffix: String,

    /// 纳入下载的补丁标签
    #[serde(rename = "PATCH_TAGS")]
    pub patch_tags: Vec<String>,
}

impl Default for UpdateTagConfig {
    fn default() -> Self {
        Self {
            patch_prefix: "p_".into(),
            u_suffix: "_u".into(),
            n_suffix: "_n".into(),
            m_suffix: "_m".into(),
            patch_tags: vec!["u".into()],
        }
    }
}
