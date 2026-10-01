use std::path::PathBuf;

use file_utils::FileHandler;
use indexmap::IndexMap;
use serde::Serialize;

use crate::error::Result;
use crate::model::{Region, RegionBytes};

#[derive(Debug, Clone)]
pub struct RegionFileStore {
    base_dir: PathBuf,
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

    pub fn save_bytes_all(&self, data: &RegionBytes) -> Result<()> {
        for (region, bytes) in data {
            self.save_bytes(region, bytes)?;
        }
        Ok(())
    }

    pub fn save_json_all<T: Serialize>(&self, data: &IndexMap<Region, T>) -> Result<()> {
        for (region, value) in data {
            self.save_json(region, value)?;
        }
        Ok(())
    }

    pub fn save_json<T: Serialize>(&self, region: &Region, value: &T) -> Result<PathBuf> {
        let path = self.path_for(region);
        FileHandler::write_json(value, &path)?;
        log::info!("[{region}] saved path= {}", path.display());
        Ok(path)
    }
}
