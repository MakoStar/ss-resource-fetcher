use std::collections::HashSet;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::generated::FileDiff;
use crate::manifest::{ManiResource, ManifestDecoder, ManifestDecryptor};
use crate::model::{Region, RegionResources, ResourceEntries, ResourceEntry};
use crate::network::{ManifestFetcher, ResourcesFetcher};

pub struct RegionTarget {
    /// 实际下载资源所用的官方区域
    region: Region,
    /// 反和谐资源输出目录
    output_dir: PathBuf,
    /// 清单下载器
    manifest_fetcher: ManifestFetcher,
    /// 清单解密器
    decryptor: ManifestDecryptor,
    /// 资源下载器
    resources_fetcher: ResourcesFetcher,
}

impl RegionTarget {
    pub fn new(app: &AppConfig, region: &Region, requester: &HttpFetcher) -> Result<Self> {
        let server = app.server_config(region).ok_or_else(|| {
            AppError::message(format!(
                "uncensor region '{region}' has no server config: neither present in [SERVERS] nor a built-in region"
            ))
        })?;

        let servers: IndexMap<Region, String> =
            [(region.clone(), server.url.clone())].into_iter().collect();
        let keys: IndexMap<Region, String> =
            [(region.clone(), server.key.clone())].into_iter().collect();
        let output_dir = PathBuf::from(&app.file_path.uncensor_output_dir);

        Ok(Self {
            region: region.clone(),
            output_dir: output_dir.clone(),
            manifest_fetcher: ManifestFetcher::new(
                app.server_route.manifest_route.clone(),
                servers.clone(),
                requester.clone(),
            ),
            decryptor: ManifestDecryptor::new(keys),
            resources_fetcher: ResourcesFetcher::new(
                app.server_route.resource_route.clone(),
                output_dir,
                servers,
                app.feature_flags.is_overwrite_resource,
                requester.clone(),
            ),
        })
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub async fn download(&self, wanted: &IndexMap<String, ManiResource>) -> Result<()> {
        let entries = self.match_source_resources(wanted).await?;
        if entries.is_empty() {
            return Err(AppError::message(format!(
                "none of the {} uncensor resources were found in region '{}' manifest",
                wanted.len(),
                self.region,
            )));
        }

        let mut resources = RegionResources::new();
        resources.insert(self.region.clone(), entries);

        self.resources_fetcher.fetch_and_save(&resources).await
    }

    async fn match_source_resources(
        &self,
        wanted: &IndexMap<String, ManiResource>,
    ) -> Result<ResourceEntries> {
        let raw = self.manifest_fetcher.fetch_one(&self.region).await?;
        let decrypted = self.decryptor.decrypt_one(&self.region, &raw)?;
        let diffs: Vec<FileDiff> = ManifestDecoder::decode_one(&self.region, &decrypted)?;

        let mut entries = ResourceEntries::new();
        let mut matched: HashSet<&str> = HashSet::with_capacity(wanted.len());

        for diff in &diffs {
            if wanted.contains_key(&diff.file_name) {
                matched.insert(diff.file_name.as_str());
                entries.insert(diff.file_name.clone(), ResourceEntry::from(diff));
            }
        }

        for missing in wanted
            .keys()
            .filter(|name| !matched.contains(name.as_str()))
        {
            log::warn!(
                "[{}] uncensor resource not found in official manifest: {missing}",
                self.region
            );
        }

        log::info!(
            "[{}] matched {} of {} uncensor resources",
            self.region,
            entries.len(),
            wanted.len()
        );

        Ok(entries)
    }
}
