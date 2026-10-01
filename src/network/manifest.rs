use indexmap::IndexMap;

use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::model::{Region, RegionBytes};

pub struct ManifestFetcher {
    manifest_route: String,
    servers: IndexMap<Region, String>,
    requester: HttpFetcher,
}

impl ManifestFetcher {
    pub fn new(
        manifest_route: String,
        servers: IndexMap<Region, String>,
        requester: HttpFetcher,
    ) -> Self {
        Self {
            manifest_route,
            servers,
            requester,
        }
    }

    pub fn from_app_config(app: &AppConfig) -> Self {
        Self::new(
            app.server_route.manifest_route.clone(),
            app.server_urls(),
            HttpFetcher::new(&app.build_fetcher_config()),
        )
    }

    pub async fn fetch_one(&self, region: &Region) -> Result<Vec<u8>> {
        logger::head!("FETCH MANIFEST {region}");

        let server_url = self
            .servers
            .get(region)
            .ok_or_else(|| AppError::UnknownRegion(region.to_string()))?;

        let url = format!("{server_url}{}", self.manifest_route);
        let bytes = self
            .requester
            .get_bytes(&url)
            .await
            .inspect_err(|err| log::error!("url={url} fetch error={err}"))?;

        logger::succ!(
            "{region} - {} - {:.2}KB",
            self.manifest_route,
            bytes.len() as f64 / 1024.0
        );

        Ok(bytes)
    }

    pub async fn fetch_all(&self) -> Result<RegionBytes> {
        let mut manifests = IndexMap::with_capacity(self.servers.len());

        for region in self.servers.keys() {
            let bytes = self.fetch_one(region).await?;
            manifests.insert(region.clone(), bytes);
        }

        Ok(manifests)
    }
}
