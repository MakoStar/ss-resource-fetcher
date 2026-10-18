use std::path::PathBuf;

use file_utils::FileHandler;
use serde::Serialize;

use crate::error::Result;
use crate::model::Region;

pub fn push_platform(dir: PathBuf, platform: &str) -> PathBuf {
    if platform.is_empty() || dir.components().any(|part| part.as_os_str() == platform) {
        return dir;
    }

    dir.join(platform)
}

#[derive(Debug, Clone)]
pub struct RegionFileStore {
    /// 根目录
    base_dir: PathBuf,
    /// 文件名
    filename: String,
    /// 平台子目录 空则不加平台层
    platform: String,
}

impl RegionFileStore {
    pub fn new(base_dir: impl Into<PathBuf>, filename: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
            filename: filename.into(),
            platform: String::new(),
        }
    }

    pub fn with_platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = platform.into();
        self
    }

    pub fn path_for(&self, region: &Region) -> PathBuf {
        let dir = self.base_dir.join(region.as_str());
        let dir = push_platform(dir, &self.platform);

        dir.join(&self.filename)
    }

    pub fn save_bytes(&self, region: &Region, data: &[u8]) -> Result<PathBuf> {
        let path = self.path_for(region);
        FileHandler::write_bytes(data, &path)?;
        log::info!("[{region}] saved {}", path.display());
        Ok(path)
    }

    pub fn save_json<T: Serialize>(&self, region: &Region, value: &T) -> Result<PathBuf> {
        let path = self.path_for(region);
        FileHandler::write_json(value, &path)?;
        log::info!("[{region}] saved {}", path.display());
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_platform_appends_once() {
        let path = push_platform(PathBuf::from("out/Metadata"), "and");

        assert_eq!(path, PathBuf::from("out/Metadata/and"));
    }

    #[test]
    fn push_platform_skips_existing_segment() {
        let path = push_platform(PathBuf::from("out/Metadata/and"), "and");

        assert_eq!(path, PathBuf::from("out/Metadata/and"));
    }

    #[test]
    fn push_platform_keeps_path_when_empty() {
        let path = push_platform(PathBuf::from("out/Metadata/CN"), "");

        assert_eq!(path, PathBuf::from("out/Metadata/CN"));
    }

    #[test]
    fn store_path_carries_region_and_platform() {
        let store = RegionFileStore::new("out/Manifest", "win_patch.json").with_platform("win");

        assert_eq!(
            store.path_for(&Region::new("CN")),
            PathBuf::from("out/Manifest/CN/win/win_patch.json")
        );
    }

    #[test]
    fn store_path_skips_duplicated_platform() {
        let store = RegionFileStore::new("out/Manifest/win", "win_patch.json").with_platform("win");

        assert_eq!(
            store.path_for(&Region::new("CN")),
            PathBuf::from("out/Manifest/win/CN/win_patch.json")
        );
    }
}
