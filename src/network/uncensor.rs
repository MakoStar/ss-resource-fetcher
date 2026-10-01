use std::collections::HashSet;
use std::path::PathBuf;

use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::generated::FileDiff;
use crate::manifest::{ManiReader, ManifestDecoder, ManifestDecryptor};
use crate::model::{Region, RegionResources, ResourceEntries, ResourceEntry};
use crate::network::{ManifestFetcher, ResourcesFetcher};

pub struct UncensorPatchFetcher {
    /// 反和谐资源清单的完整下载地址
    manifest_url: String,
    /// 清单落盘路径
    manifest_output_path: PathBuf,
    /// 实际下载资源所对应的区域键
    source_region: Region,
    /// 反和谐资源清单获取器
    manifest_fetcher: ManifestFetcher,
    /// 反和谐资源清单解密器
    decryptor: ManifestDecryptor,
    /// 反和谐资源下载器
    resources_fetcher: ResourcesFetcher,
}

impl UncensorPatchFetcher {
    pub fn from_app_config(app: &AppConfig) -> Result<Self> {
        let source_region = Region::new(&app.uncensor.source_region);

        let server = app.server_config(&source_region).ok_or_else(|| {
            AppError::message(format!(
                "uncensor source region '{source_region}' is not configured and has no built-in server"
            ))
        })?;

        let servers: IndexMap<Region, String> = [(source_region.clone(), server.url.clone())]
            .into_iter()
            .collect();

        let keys: IndexMap<Region, String> = [(source_region.clone(), server.key.clone())]
            .into_iter()
            .collect();

        let manifest_url = app.uncensor.manifest_url();
        let manifest_file = manifest_name(&app.uncensor.manifest_route);
        let output_dir = PathBuf::from(&app.file_path.uncensor_output_dir);

        let requester = HttpFetcher::new(&app.build_fetcher_config());

        Ok(Self {
            manifest_url,
            manifest_output_path: output_dir.join(manifest_file),
            source_region,
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
                requester,
            ),
        })
    }

    pub async fn fetch_all(&self) -> Result<()> {
        logger::head!("FETCH UNCENSOR MANIFEST");

        let wanted = self.download_resource_names().await?;
        if wanted.is_empty() {
            log::warn!("uncensor manifest lists no resources, nothing to do");
            return Ok(());
        }
        log::info!("uncensor manifest lists {} resources", wanted.len());

        let entries = self.match_source_resources(&wanted).await?;
        if entries.is_empty() {
            return Err(AppError::message(format!(
                "none of the {} uncensor resources were found in region '{}' manifest",
                wanted.len(),
                self.source_region,
            )));
        }

        let mut resources = RegionResources::new();
        resources.insert(self.source_region.clone(), entries);

        self.resources_fetcher.fetch_and_save(&resources).await
    }

    async fn download_resource_names(&self) -> Result<HashSet<String>> {
        let url = &self.manifest_url;
        log::info!("downloading uncensor manifest: {url}");

        let bytes = self.manifest_fetcher.fetch_url(url).await?;
        FileHandler::write_bytes(&bytes, &self.manifest_output_path)?;
        log::info!(
            "saved uncensor manifest: {} ({:.2}KB)",
            self.manifest_output_path.display(),
            bytes.len() as f64 / 1024.0
        );

        let reader = ManiReader::new(&self.manifest_output_path)?;
        let names: HashSet<String> = reader
            .resource_names()
            .into_iter()
            .map(str::to_string)
            .collect();

        Ok(names)
    }

    async fn match_source_resources(&self, wanted: &HashSet<String>) -> Result<ResourceEntries> {
        let raw = self.manifest_fetcher.fetch_one(&self.source_region).await?;
        let decrypted = self.decryptor.decrypt_one(&self.source_region, &raw)?;
        let diffs: Vec<FileDiff> = ManifestDecoder::decode_one(&self.source_region, &decrypted)?;

        let mut entries = ResourceEntries::new();
        let mut matched: HashSet<&str> = HashSet::with_capacity(wanted.len());

        for diff in &diffs {
            if wanted.contains(&diff.file_name) {
                matched.insert(diff.file_name.as_str());
                entries.insert(diff.file_name.clone(), ResourceEntry::from(diff));
            }
        }

        for missing in wanted
            .iter()
            .filter(|name| !matched.contains(name.as_str()))
        {
            log::warn!(
                "[{}] uncensor resource not found in official manifest: {missing}",
                self.source_region
            );
        }

        log::info!(
            "[{}] matched {} of {} uncensor resources",
            self.source_region,
            entries.len(),
            wanted.len()
        );

        Ok(entries)
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
