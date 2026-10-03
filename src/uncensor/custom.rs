use std::path::Path;

use file_utils::FileHandler;
use indexmap::IndexSet;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::Result;
use crate::model::{CustomFileList, Region};
use crate::uncensor::region::RegionTarget;

pub struct CustomFilesTarget {
    /// 需要下载的资源文件名
    files: IndexSet<String>,
    /// 复用区域模式的下载逻辑
    region_target: RegionTarget,
}

impl CustomFilesTarget {
    pub fn new(
        app: &AppConfig,
        path: &Path,
        region_override: Option<&Region>,
        requester: &HttpFetcher,
    ) -> Result<Self> {
        let list = Self::load(path)?;
        let region = match region_override {
            Some(region) => region.clone(),
            None => Region::new(list.region.trim().to_uppercase()),
        };

        let files: IndexSet<String> = list
            .files
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();

        Ok(Self {
            files,
            region_target: RegionTarget::new(app, &region, requester)?,
        })
    }

    pub fn output_dir(&self) -> &Path {
        self.region_target.output_dir()
    }

    pub async fn download(&self) -> Result<()> {
        if self.files.is_empty() {
            log::warn!("custom uncensor file list is empty, nothing to do");
            return Ok(());
        }

        logger::head!("FETCH UNCENSOR RESOURCES FROM CUSTOM FILE LIST");
        log::info!("custom file list lists {} resources", self.files.len());

        self.region_target.download_names(&self.files).await
    }

    fn load(path: &Path) -> Result<CustomFileList> {
        Ok(FileHandler::read_json(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::CustomFileList;

    #[test]
    fn parses_custom_file_list() {
        let list: CustomFileList = serde_json::from_str(
            r#"{ "region": "TW", "files": ["char_2d_14901.unity3d", "char_2d_14902.unity3d"] }"#,
        )
        .unwrap();

        assert_eq!(list.region, "TW");
        assert_eq!(
            list.files,
            vec!["char_2d_14901.unity3d", "char_2d_14902.unity3d"]
        );
    }
}
