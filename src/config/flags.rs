use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeatureFlags {
    /// 是否使用默认区域键
    #[serde(rename = "IS_USE_DEFAULT_REGION")]
    pub is_use_default_region: bool,

    /// 是否保存原始清单
    #[serde(rename = "IS_SAVE_RAW_MANIFEST")]
    pub is_save_raw_manifest: bool,

    /// 是否保存解密后的清单
    #[serde(rename = "IS_SAVE_DECRYPTED_MANIFEST")]
    pub is_save_decrypted_manifest: bool,

    /// 是否保存解码后的清单
    #[serde(rename = "IS_SAVE_DECODED_MANIFEST")]
    pub is_save_decoded_manifest: bool,

    /// 是否提取补丁清单
    #[serde(rename = "IS_EXTRACT_PATCH_MANIFEST")]
    pub is_extract_patch_manifest: bool,

    /// 是否保存补丁清单
    #[serde(rename = "IS_SAVE_PATCH_MANIFEST")]
    pub is_save_patch_manifest: bool,

    /// 是否下载资源文件
    #[serde(rename = "IS_DOWNLOAD_RESOURCE")]
    pub is_download_resource: bool,

    /// 是否覆盖资源文件
    #[serde(rename = "IS_OVERWRITE_RESOURCE")]
    pub is_overwrite_resource: bool,

    /// 是否合并补丁资源
    #[serde(rename = "IS_MERGE_PATCH")]
    pub is_merge_patch: bool,

    /// 是否保存版本文件
    #[serde(rename = "IS_SAVE_VERSION_FILE")]
    pub is_save_version_file: bool,

    /// 是否提取补丁版本信息
    #[serde(rename = "IS_EXTRACT_PATCH_VERSION")]
    pub is_extract_patch_version: bool,

    /// 是否启用资源正则过滤
    #[serde(rename = "IS_USE_RESOURCE_REGEX")]
    pub is_use_resource_regex: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            is_use_default_region: true,
            is_save_raw_manifest: false,
            is_save_decrypted_manifest: false,
            is_save_decoded_manifest: true,
            is_extract_patch_manifest: true,
            is_save_patch_manifest: true,
            is_download_resource: true,
            is_overwrite_resource: false,
            is_merge_patch: true,
            is_save_version_file: true,
            is_extract_patch_version: true,
            is_use_resource_regex: false,
        }
    }
}
