use std::path::{Path, PathBuf};

use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::Result;
use crate::manifest::ManiResource;
use crate::model::{DownloadProgress, SEPARATOR};

pub struct DefaultUrlTarget {
    /// 资源基础地址
    resource_base_url: String,
    /// 资源输出目录
    output_dir: PathBuf,
    /// 是否强制覆盖已存在的文件
    overwrite: bool,
    /// 资源下载器
    requester: HttpFetcher,
}

impl DefaultUrlTarget {
    pub fn new(app: &AppConfig, requester: &HttpFetcher) -> Self {
        Self {
            resource_base_url: app.uncensor.resource_base_url(),
            output_dir: PathBuf::from(&app.file_path.uncensor_default_output_dir),
            overwrite: app.feature_flags.is_overwrite_resource,
            requester: requester.clone(),
        }
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub async fn download(&self, wanted: &IndexMap<String, ManiResource>) -> Result<()> {
        if self.overwrite {
            logger::tips!("IS_OVERWRITE_RESOURCE is enabled, resources will re-download !!!");
        }

        logger::head!("FETCHING UNCENSOR RESOURCES {}", self.resource_base_url);

        let total = wanted.len();
        for (index, (name, resource)) in wanted.iter().enumerate() {
            let progress = DownloadProgress::new(index + 1, total);
            self.download_one(name, resource, &progress).await?;

            if index + 1 < total {
                log::debug!("{SEPARATOR}");
            }
        }

        logger::tips!("uncensor pack done, {total} file(s) downloaded");

        Ok(())
    }

    async fn download_one(
        &self,
        name: &str,
        resource: &ManiResource,
        progress: &DownloadProgress,
    ) -> Result<()> {
        let prefix = progress.prefix();
        let save_path = self.output_dir.join(name);

        if self.is_cached(&save_path, resource, &prefix, name) {
            return Ok(());
        }

        let url = format!("{}/{}", self.resource_base_url, name);
        let bytes = self
            .requester
            .get_bytes(&url)
            .await
            .inspect_err(|err| log::error!("url={url} fetch error={err}"))?;

        FileHandler::write_bytes(&bytes, &save_path)?;
        log::info!("{} {name} - {:.2}KB", prefix, bytes.len() as f64 / 1024.0);

        match FileHandler::verify_md5(&save_path, &resource.hash) {
            Ok(()) => log::info!("{} MD5 - {}", prefix, resource.hash.to_uppercase()),
            Err(err) => log::warn!("{} MD5 mismatch: {err}", prefix),
        }

        Ok(())
    }

    fn is_cached(
        &self,
        save_path: &Path,
        resource: &ManiResource,
        prefix: &str,
        name: &str,
    ) -> bool {
        if self.overwrite {
            return false;
        }

        if !save_path.is_file() {
            logger::step!("{prefix} {name} not found locally, will download");
            return false;
        }

        if FileHandler::verify_md5(save_path, &resource.hash).is_ok() {
            logger::step!("{prefix} SKIP (cached): {name}");
            true
        } else {
            log::warn!("{prefix} {name} MD5 mismatch, will re-download");
            false
        }
    }
}
