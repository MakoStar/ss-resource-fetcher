use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilePathConfig {
    /// 根输出目录
    #[serde(rename = "ROOT_OUTPUT_DIR")]
    pub root_output_dir: String,

    /// 清单文件输出目录
    #[serde(rename = "MANIFEST_OUTPUT_DIR")]
    pub manifest_output_dir: String,

    /// 版本文件输出目录
    #[serde(rename = "VERSIONS_OUTPUT_DIR")]
    pub versions_output_dir: String,

    /// 补丁文件输出目录
    #[serde(rename = "PATCH_OUTPUT_DIR")]
    pub patch_output_dir: String,

    /// 解包文件输出目录
    #[serde(rename = "UNPACK_OUTPUT_DIR")]
    pub unpack_output_dir: String,

    /// 反和谐资源输出目录
    #[serde(rename = "UNCENSOR_OUTPUT_DIR")]
    pub uncensor_output_dir: String,

    /// 反和谐默认地址资源输出目录
    #[serde(rename = "UNCENSOR_DEFAULT_OUTPUT_DIR")]
    pub uncensor_default_output_dir: String,

    /// launcher 资源输出目录
    #[serde(rename = "LAUNCHER_OUTPUT_DIR")]
    pub launcher_output_dir: String,
}

impl Default for FilePathConfig {
    fn default() -> Self {
        Self {
            root_output_dir: "./output/".into(),
            manifest_output_dir: "./output/Manifest".into(),
            versions_output_dir: "./output/Manifest".into(),
            patch_output_dir: "./output/Metadata".into(),
            unpack_output_dir: "./output/Unpack".into(),
            uncensor_output_dir: "./output/Uncensor".into(),
            uncensor_default_output_dir: "./output/UncensorDefault".into(),
            launcher_output_dir: "./output/Launcher".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileNameConfig {
    /// 原始清单文件名
    #[serde(rename = "MANIFEST_RAW_FILE")]
    pub manifest_raw_file: String,

    /// 解密后的清单文件名
    #[serde(rename = "MANIFEST_DECRYPT_FILE")]
    pub manifest_decrypt_file: String,

    /// 解码后的清单文件名
    #[serde(rename = "MANIFEST_DECODED_FILE")]
    pub manifest_decoded_file: String,

    /// 补丁清单文件名
    #[serde(rename = "PATCH_MANIFEST_FILE")]
    pub patch_manifest_file: String,

    /// 版本文件名
    #[serde(rename = "VERSION_FILE")]
    pub version_file: String,

    /// 更新根清单文件名
    #[serde(rename = "UPDATE_ROOT_MANI_FILE")]
    pub update_root_mani_file: String,

    /// 资源补丁标签前缀
    #[serde(rename = "UPDATE_RESOURCE_PATCH_TAG_PREFIX")]
    pub update_resource_patch_tag_prefix: String,

    /// 资源补丁标签后缀
    #[serde(rename = "UPDATE_RESOURCE_PATCH_TAG_SUFFIX")]
    pub update_resource_patch_tag_suffix: String,

    /// 清单补丁标签后缀
    #[serde(rename = "UPDATE_MANIFEST_PATCH_TAG_SUFFIX")]
    pub update_manifest_patch_tag_suffix: String,

    /// 清单扩展名
    #[serde(rename = "UPDATE_MANIFEST_EXTENSION")]
    pub update_manifest_extension: String,

    /// 第一个清单补丁文件名
    #[serde(rename = "UPDATE_FIRST_MANIFEST_PATCH_NAME")]
    pub update_first_manifest_patch_name: String,

    /// 附加清单文件名
    #[serde(rename = "UPDATE_ADD_MANIFEST_NAME")]
    pub update_add_manifest_name: String,

    /// launcher 清单文件名
    #[serde(rename = "LAUNCHER_MANIFEST_FILE")]
    pub launcher_manifest_file: String,
}

impl Default for FileNameConfig {
    fn default() -> Self {
        Self {
            manifest_raw_file: "win.html".into(),
            manifest_decrypt_file: "win.bytes".into(),
            manifest_decoded_file: "resource_manifest.json".into(),
            patch_manifest_file: "patch_manifest.json".into(),
            version_file: "version.json".into(),
            update_root_mani_file: "ss_win.mani".into(),
            update_resource_patch_tag_prefix: "p_".into(),
            update_resource_patch_tag_suffix: "_u".into(),
            update_manifest_patch_tag_suffix: "_m".into(),
            update_manifest_extension: ".mani".into(),
            update_first_manifest_patch_name: "p_1_m.ss_win.mani".into(),
            update_add_manifest_name: "ss_win_add.mani".into(),
            launcher_manifest_file: "launcher_manifest.json".into(),
        }
    }
}
