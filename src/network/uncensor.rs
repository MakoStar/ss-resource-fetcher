use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::Result;
use crate::manifest::{ManiReader, ManiResource};
use crate::model::UncensorSource;
use crate::uncensor::UncensorTarget;

pub struct UncensorPatchFetcher {
    /// 第三方反和谐清单的完整下载地址
    manifest_url: String,
    /// 清单落盘路径 跟随当前模式的输出目录
    manifest_output_path: std::path::PathBuf,
    /// 清单下载器
    requester: HttpFetcher,
    /// 资源下载方式
    target: UncensorTarget,
}

impl UncensorPatchFetcher {
    pub fn from_app_config(app: &AppConfig, source: &UncensorSource) -> Result<Self> {
        let requester = HttpFetcher::new(&app.build_fetcher_config());
        let target = UncensorTarget::from_app_config(app, source, &requester)?;

        Ok(Self {
            manifest_url: app.uncensor.manifest_url(),
            manifest_output_path: target.output_dir().join(app.uncensor.manifest_file_name()),
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

        let reader = ManiReader::new(&self.manifest_output_path, None)?;
        let entries: IndexMap<String, ManiResource> = reader
            .resources()
            .into_iter()
            .map(|res| (res.file.clone(), res.clone()))
            .collect();

        Ok(entries)
    }
}
