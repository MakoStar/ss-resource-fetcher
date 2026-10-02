use std::path::Path;

use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::Result;
use crate::manifest::ManiResource;
use crate::model::{Region, UncensorSource};
use crate::uncensor::custom::CustomFilesTarget;
use crate::uncensor::default::DefaultUrlTarget;
use crate::uncensor::region::RegionTarget;

pub enum UncensorTarget {
    /// 与官方区域清单匹配后从区域服务器下载
    Region(Box<RegionTarget>),
    /// 直接从反和谐清单所在目录下载
    DefaultUrl(Box<DefaultUrlTarget>),
    /// 读取本地自定义文件列表后，与官方区域清单匹配下载
    CustomFiles(Box<CustomFilesTarget>),
}

impl UncensorTarget {
    pub fn from_app_config(
        app: &AppConfig,
        source: &UncensorSource,
        requester: &HttpFetcher,
    ) -> Result<Self> {
        match source {
            UncensorSource::DefaultUrl => Ok(Self::DefaultUrl(Box::new(DefaultUrlTarget::new(
                app, requester,
            )))),
            UncensorSource::Region(region) => Ok(Self::Region(Box::new(RegionTarget::new(
                app, region, requester,
            )?))),
            UncensorSource::ConfigRegion => {
                let region = Region::new(&app.uncensor.source_region);
                Ok(Self::Region(Box::new(RegionTarget::new(
                    app, &region, requester,
                )?)))
            }
            UncensorSource::CustomFiles { path, region } => Ok(Self::CustomFiles(Box::new(
                CustomFilesTarget::new(app, path, region.as_ref(), requester)?,
            ))),
        }
    }

    pub fn output_dir(&self) -> &Path {
        match self {
            Self::Region(target) => target.output_dir(),
            Self::DefaultUrl(target) => target.output_dir(),
            Self::CustomFiles(target) => target.output_dir(),
        }
    }

    pub async fn download(&self, wanted: &IndexMap<String, ManiResource>) -> Result<()> {
        match self {
            Self::Region(target) => target.download(wanted).await,
            Self::DefaultUrl(target) => target.download(wanted).await,
            Self::CustomFiles(_) => unreachable!("custom files are handled by the fetcher"),
        }
    }
}
