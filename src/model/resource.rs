use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::generated::FileDiff;
use crate::model::Region;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResourceEntry {
    /// 资源文件名
    pub file_name: String,
    /// 文件校验值
    pub hash: String,
    /// 资源版本
    pub version: u64,
    /// 附加路径
    pub additional_path: String,
}

impl From<&FileDiff> for ResourceEntry {
    fn from(diff: &FileDiff) -> Self {
        Self {
            file_name: diff.file_name.clone(),
            hash: diff.hash.clone(),
            version: diff.version as u64,
            additional_path: diff.additional_path.clone(),
        }
    }
}

pub type ResourceEntries = IndexMap<String, ResourceEntry>;
pub type RegionResources = IndexMap<Region, ResourceEntries>;
pub type RegionBytes = IndexMap<Region, Vec<u8>>;
pub type RegionFileDiffs = IndexMap<Region, Vec<FileDiff>>;
