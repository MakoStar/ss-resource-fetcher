use std::path::PathBuf;

use file_utils::FileHandler;
use serde::Serialize;

use crate::error::Result;
use crate::model::Region;

#[derive(Debug, Clone)]
pub struct RegionFileStore {
    /// 根目录
    base_dir: PathBuf,
    /// 文件名
    filename: String,
}

impl RegionFileStore {
    pub fn new(base_dir: impl Into<PathBuf>, filename: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
            filename: filename.into(),
        }
    }

    pub fn path_for(&self, region: &Region) -> PathBuf {
        self.base_dir.join(region.as_str()).join(&self.filename)
    }

    pub fn save_bytes(&self, region: &Region, data: &[u8]) -> Result<PathBuf> {
        let path = self.path_for(region);
        FileHandler::write_bytes(data, &path)?;
        log::info!("[{region}] saved path= {}", path.display());
        Ok(path)
    }

    pub fn save_json<T: Serialize>(&self, region: &Region, value: &T) -> Result<PathBuf> {
        let path = self.path_for(region);
        FileHandler::write_json(value, &path)?;
        log::info!("[{region}] saved path= {}", path.display());
        Ok(path)
    }
}
