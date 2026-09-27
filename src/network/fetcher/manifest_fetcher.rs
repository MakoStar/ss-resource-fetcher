use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use indexmap::IndexMap;

use file_utils::FileHandler;
use network_manager::HttpFetcher;
use network_manager::FetcherConfig;

use crate::config::AppConfig;
use crate::network::ManifestError;
use crate::network::TManifestResult;

/// 服务器资源清单获取器
pub struct ManifestFetcher {
    meta_win_route: String,
    raw_output_dir: String,
    raw_filename: String,
    requester: HttpFetcher,
    app_config: &'static AppConfig,
}

impl ManifestFetcher {
    pub fn new() -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let fetcher_config: FetcherConfig = app_config.build_fetcher_config();
        let meta_win_route: String = app_config.server_route.manifest_route.clone();
        let raw_output_dir: String = app_config.file_path.manifest_output_dir.clone();
        let raw_filename: String = app_config.file_name.manifest_raw_file.clone();
        let requester: HttpFetcher = HttpFetcher::new(&fetcher_config);
        
        Self { 
            meta_win_route,
            raw_output_dir,
            raw_filename,
            requester, 
            app_config,
        }
    }

    /// 保存清单原始响应数据的路径
    fn raw_file_path(&self, region: &str) -> PathBuf {
        Path::new(&self.raw_output_dir)
            .join(region)
            .join(&self.raw_filename)
    }

    /// 保存清单原始数据文件
    fn save_raw_manifests(&self, region: &str, data: &Vec<u8>) -> Result<()> {
        let save_path: PathBuf = self.raw_file_path(region);
        FileHandler::write_bytes(&data, &save_path)?;
        log::info!("[{}] Saved path= {}", region, save_path.display());

        Ok(())
    }

    /// 获取单个区域的资源清单
    pub async fn fetch_manifest(&self, region: &str) -> TManifestResult<Vec<u8>> {
        logger::head!("FETCHING MANIFEST {}", region);
        
        let Some(region_config) = self.app_config.get_server(region) else {
            return Err(ManifestError::UnknownRegion {
                region: region.to_string(),
            });
        };

        let url: String = format!("{}{}", region_config.url, &self.meta_win_route);
        let raw_bytes: Vec<u8> = self.requester
            .get_bytes(url.as_str())
            .await
            .inspect_err(|e| {
                log::error!("region={region} url={url} fetch error={e}");
            })?;

        logger::succ!(
            "{region} - {} - {:.2}KB", 
            self.raw_filename, 
            raw_bytes.len() as f64 / 1024.0
        );

        if self.app_config.feature_flags.is_save_raw_manifest {
            self.save_raw_manifests(region, &raw_bytes)
                .map_err(|e| ManifestError::Other(e.to_string()))?;
        }

        Ok(raw_bytes)
    }

    /// 获取全部区域的清单
    pub async fn fetch_all_manifest(&self) -> TManifestResult<IndexMap<String, Vec<u8>>> {
        let mut map: IndexMap<String, Vec<u8>> = IndexMap::new();
        for (region, _server) in &self.app_config.servers {
            let raw_bytes: Vec<u8> = self.fetch_manifest(region).await?;
            map.insert(region.to_string(), raw_bytes);
        }
        Ok(map)
    }
}

impl Default for ManifestFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use file_utils::FileHandler;

    #[tokio::test]
    async fn test_func() -> Result<(), ManifestError> {
        logger::init_logger!();

        let fetcher: ManifestFetcher = ManifestFetcher::new();
        let manifests: IndexMap<String, Vec<u8>> = fetcher.fetch_all_manifest().await?;

        for (region, manifest_data) in &manifests {
            let save_path: std::path::PathBuf = FileHandler::resolve_path(
                Path::new("./test")
                .join(region)
                // .join(FileNameConfigs::MANIFEST_RAW_FILE))?;
                .join("test.html"))?;
            FileHandler::write_bytes(manifest_data, save_path)?;
        }

        assert!(!manifests.is_empty());

        Ok(())
    }
}
