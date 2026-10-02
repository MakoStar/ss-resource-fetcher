mod custom_files;
mod default_url;
mod region;

use std::path::{Path, PathBuf};

use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::Result;
use crate::manifest::{ManiReader, ManiResource};
use crate::model::{Region, UncensorSource};
use crate::network::uncensor::custom_files::CustomFilesTarget;
use crate::network::uncensor::default_url::DefaultUrlTarget;
use crate::network::uncensor::region::RegionTarget;

pub struct UncensorPatchFetcher {
    /// 反和谐资源清单的完整下载地址
    manifest_url: String,
    /// 清单落盘路径
    manifest_output_path: PathBuf,
    /// 清单下载器
    requester: HttpFetcher,
    /// 资源下载方式
    target: UncensorTarget,
}

enum UncensorTarget {
    /// 与官方区域清单匹配后从区域服务器下载
    Region(Box<RegionTarget>),
    /// 直接从反和谐资源清单服务器下载
    DefaultUrl(Box<DefaultUrlTarget>),
    /// 读取本地自定义文件列表后，与官方区域清单匹配下载
    CustomFiles(Box<CustomFilesTarget>),
}

impl UncensorPatchFetcher {
    pub fn from_app_config(app: &AppConfig, source: &UncensorSource) -> Result<Self> {
        let requester = HttpFetcher::new(&app.build_fetcher_config());
        let target = UncensorTarget::from_app_config(app, source, &requester)?;

        Ok(Self {
            manifest_url: app.uncensor.manifest_url(),
            manifest_output_path: target
                .output_dir()
                .join(manifest_name(&app.uncensor.manifest_route)),
            requester,
            target,
        })
    }

    pub async fn fetch_all(&self) -> Result<()> {
        if let UncensorTarget::CustomFiles(target) = &self.target {
            return target.download().await;
        }

        self.fetch_from_manifest().await
    }

    async fn fetch_from_manifest(&self) -> Result<()> {
        logger::head!("FETCH UNCENSOR MANIFEST");

        let wanted = self.download_resource_entries().await?;
        if wanted.is_empty() {
            log::warn!("uncensor manifest lists no resources, nothing to do");
            return Ok(());
        }
        log::info!("uncensor manifest lists {} resources", wanted.len());

        self.target.download(&wanted).await
    }

    async fn download_resource_entries(&self) -> Result<IndexMap<String, ManiResource>> {
        let url = &self.manifest_url;
        log::info!("downloading uncensor manifest: {url}");

        let bytes = self
            .requester
            .get_bytes(url)
            .await
            .inspect_err(|err| log::error!("url={url} fetch error={err}"))?;

        FileHandler::write_bytes(&bytes, &self.manifest_output_path)?;
        log::info!(
            "saved uncensor manifest: {} ({:.2}KB)",
            self.manifest_output_path.display(),
            bytes.len() as f64 / 1024.0
        );

        let reader = ManiReader::new(&self.manifest_output_path)?;
        let entries: IndexMap<String, ManiResource> = reader
            .resources()
            .into_iter()
            .map(|res| (res.file.clone(), res.clone()))
            .collect();

        Ok(entries)
    }
}

impl UncensorTarget {
    fn from_app_config(
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

    fn output_dir(&self) -> &Path {
        match self {
            Self::Region(target) => target.output_dir(),
            Self::DefaultUrl(target) => target.output_dir(),
            Self::CustomFiles(target) => target.output_dir(),
        }
    }

    async fn download(&self, wanted: &IndexMap<String, ManiResource>) -> Result<()> {
        match self {
            Self::Region(target) => target.download(wanted).await,
            Self::DefaultUrl(target) => target.download(wanted).await,
            Self::CustomFiles(_) => unreachable!("custom files are handled in fetch_all"),
        }
    }
}

fn manifest_name(manifest_route: &str) -> &str {
    manifest_route
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("ss_win.mani")
}

#[cfg(test)]
mod tests {
    use super::manifest_name;

    #[test]
    fn extracts_manifest_file_name() {
        assert_eq!(manifest_name("res/win/ss_win.mani"), "ss_win.mani");
        assert_eq!(manifest_name("/res/win/ss_win.mani"), "ss_win.mani");
        assert_eq!(manifest_name("ss_win.mani"), "ss_win.mani");
        assert_eq!(manifest_name(""), "ss_win.mani");
    }
}
