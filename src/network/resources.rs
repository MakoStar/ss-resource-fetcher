use std::path::{Path, PathBuf};

use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::model::{Region, RegionResources, ResourceEntry};

#[derive(Clone, Copy)]
struct Progress {
    current: usize,
    total: usize,
}

impl Progress {
    fn new(current: usize, total: usize) -> Self {
        Self { current, total }
    }

    fn prefix(&self) -> String {
        format!("[{}/{}]", self.current, self.total)
    }
}

pub struct ResourcesFetcher {
    resource_route: String,
    output_dir: PathBuf,
    servers: IndexMap<Region, String>,
    overwrite: bool,
    requester: HttpFetcher,
}

impl ResourcesFetcher {
    pub fn new(
        resource_route: String,
        output_dir: impl Into<PathBuf>,
        servers: IndexMap<Region, String>,
        overwrite: bool,
        requester: HttpFetcher,
    ) -> Self {
        Self {
            resource_route,
            output_dir: output_dir.into(),
            servers,
            overwrite,
            requester,
        }
    }

    pub fn from_app_config(app: &AppConfig) -> Self {
        Self::new(
            app.server_route.resource_route.clone(),
            app.file_path.patch_output_dir.clone(),
            app.server_urls(),
            app.feature_flags.is_overwrite_resource,
            HttpFetcher::new(&app.build_fetcher_config()),
        )
    }

    pub async fn fetch_and_save(&self, manifest: &RegionResources) -> Result<()> {
        if self.overwrite {
            logger::tips!("IS_OVERWRITE_RESOURCE is enabled, resources will re-download !!!");
        }

        for (region, resources) in manifest {
            logger::head!("FETCHING RESOURCES {region}");

            let server_url = self
                .servers
                .get(region)
                .ok_or_else(|| AppError::MissingServer(region.to_string()))?
                .as_str();

            let total = resources.len();
            for (index, (resource_name, entry)) in resources.iter().enumerate() {
                self.fetch_and_save_one(
                    region,
                    server_url,
                    resource_name,
                    entry,
                    Progress::new(index + 1, total),
                )
                .await?;

                if index + 1 < total {
                    log::debug!("{}", "-".repeat(64));
                }
            }
        }

        Ok(())
    }

    async fn fetch_and_save_one(
        &self,
        region: &Region,
        server_url: &str,
        resource_name: &str,
        entry: &ResourceEntry,
        progress: Progress,
    ) -> Result<()> {
        let save_path = self.output_path(region, resource_name);

        if self.is_cached(&save_path, &entry.hash, region, resource_name, progress) {
            return Ok(());
        }

        let url = self.build_url(
            server_url,
            entry.version,
            &entry.additional_path,
            resource_name,
        );
        let data = self.download(region, &url, resource_name).await?;

        FileHandler::write_bytes(&data, &save_path)?;
        FileHandler::verify_md5(&save_path, &entry.hash)?;

        log::info!("MD5 - {}", entry.hash.to_uppercase());

        Ok(())
    }

    fn build_url(
        &self,
        server_url: &str,
        version: u64,
        additional_path: &str,
        resource_name: &str,
    ) -> String {
        let version = version.to_string();
        [
            server_url.trim_end_matches('/'),
            self.resource_route.trim_matches('/'),
            version.as_str(),
            additional_path.trim_matches('/'),
            resource_name.trim_start_matches('/'),
        ]
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
    }

    fn output_path(&self, region: &Region, resource_name: &str) -> PathBuf {
        self.output_dir.join(region.as_str()).join(resource_name)
    }

    fn is_cached(
        &self,
        save_path: &Path,
        expected_hash: &str,
        region: &Region,
        resource_name: &str,
        progress: Progress,
    ) -> bool {
        if self.overwrite {
            return false;
        }

        if !save_path.is_file() {
            logger::step!(
                "{} {region} {resource_name} not found locally, will download",
                progress.prefix()
            );
            return false;
        }

        if FileHandler::verify_md5(save_path, expected_hash).is_ok() {
            logger::step!(
                "{} SKIP (cached): {}",
                progress.prefix(),
                save_path.display()
            );
            true
        } else {
            log::warn!(
                "{} {region} {resource_name} MD5 mismatch, will re-download",
                progress.prefix()
            );
            false
        }
    }

    async fn download(&self, region: &Region, url: &str, resource_name: &str) -> Result<Vec<u8>> {
        let bytes = self
            .requester
            .get_bytes(url)
            .await
            .inspect_err(|err| log::error!("region={region} url={url} fetch error={err}"))?;

        log::info!(
            "{region} - {resource_name} - {:.2}KB",
            bytes.len() as f64 / 1024.0
        );

        Ok(bytes)
    }
}
