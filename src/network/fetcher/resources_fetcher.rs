use std::path::Path;
use std::path::PathBuf;

use file_utils::FileHandler;
use network_manager::HttpFetcher;
use network_manager::FetcherConfig;

use crate::config::AppConfig;
use crate::network::TResourceResult;
use crate::extractor::TManifestEntryIndexMap;


/// 资源获取器
pub struct ResourcesFetcher {
    resource_route: String,
    patch_output_dir: String,
    requester: HttpFetcher,
    app_config: &'static AppConfig,
}

impl ResourcesFetcher {
    pub fn new() -> Self {
        let app_config: &AppConfig = AppConfig::get();
        let fetcher_config: FetcherConfig = app_config.request.build_fetcher_config();
        Self {
            resource_route: app_config.server_route.resource_route.clone(),
            patch_output_dir: app_config.file_path.patch_output_dir.clone(),
            requester: HttpFetcher::new(&fetcher_config),
            app_config,
        }
    }

    /// 构建资源请求地址
    fn build_resource_url(&self, s_url: &str, r_ver: u64, s_path: &str, r_name: &str) -> String {
        let resource_version: String = r_ver.to_string();
        let segments: Vec<&str> = [
            s_url.trim_end_matches('/'),
            self.resource_route.trim_matches('/'),
            resource_version.as_str(),
            s_path.trim_matches('/'),
            r_name.trim_start_matches('/'),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
        segments.join("/")
    }

    /// 补丁资源文件保存路径
    fn patch_output_path(&self, region: &str, patch_name: &str) -> PathBuf {
        Path::new(&self.patch_output_dir)
            .join(region)
            .join(patch_name)

    }

    /// 通过资源请求地址获取资源文件
    pub async fn fetch_resource(&self, region: &str, url: &str, res_name: &str) -> TResourceResult<Vec<u8>> {
        let raw_bytes: Vec<u8> = self
            .requester
            .get_bytes(url)
            .await
            .inspect_err(|e| {
                log::error!("region={region} url={url} fetch error={e}");
            })?;
        log::info!("{region} - {res_name} - {:.2}KB", raw_bytes.len() as f64 / 1024.0);

        Ok(raw_bytes)
    }

    /// 检查缓存并处理，返回 true 表示应该跳过下载
    fn check_and_handle_cache(&self, save_path: &Path, expected_hash: &str, region: &str, res_name: &str, current: usize, total: usize) -> bool {
        if self.app_config.feature_flags.is_overwrite_resource {
            return false;
        }

        if !save_path.exists() {
            logger::step!("[{}/{}] {region} {res_name} not found locally, will download", current, total);
            return false;
        }

        if FileHandler::verify_md5(save_path, expected_hash).is_ok() {
            logger::step!("[{}/{}] SKIP (cached): {}", current, total, save_path.display());
            return true;
        } else {
            log::warn!("[{}/{}] {region} {res_name} MD5 mismatch, will re-download", current, total);
            return false;
        }
    }

    /// 从清单数据开始请求获取和保存资源 (不使用并发，那玩意搞得日志错乱)
    pub async fn fetch_and_save_from_manifest(&self, manifest_data: &TManifestEntryIndexMap) -> TResourceResult<()> {
        if self.app_config.feature_flags.is_overwrite_resource {
            log::debug!("");
            logger::tips!("IS_OVERWRITE_RESOURCE is enabled, resources will re-download !!!");
            log::debug!("");
        }
        for (region, resources) in manifest_data {
            logger::head!("FETCHING RESOURCES {}", region);
            let Some(server_config) = self.app_config.get_server(region) else {
                return Err(anyhow::anyhow!("{}: server config not found", region).into());
            };
            for (idx, (res_name, res_map)) in resources.iter().enumerate() {
                let is_last: bool = idx == resources.len() - 1;
                let save_path: PathBuf = self.patch_output_path(region, res_name);

                if self.check_and_handle_cache(
                    &save_path, 
                    res_map.hash.as_str(), 
                    region, 
                    res_name, 
                    idx + 1,
                    resources.len()
                ) {
                    continue;
                }
                
                let res_url: String = self.build_resource_url(
                    &server_config.url,
                    res_map.version,
                    &res_map.additional_path,
                    res_name,
                );

                let data: Vec<u8> = self.fetch_resource(region, &res_url, res_name).await?;
                
                FileHandler::write_bytes(&data, &save_path)?;
                FileHandler::verify_md5(&save_path, res_map.hash.as_str())?;
                
                log::info!("MD5 - {}", res_map.hash.as_str().to_uppercase());
                
                if !is_last {
                    log::debug!("{}", "-".repeat(64));
                }
            }
        }

        Ok(())
    }
}

impl Default for ResourcesFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use indexmap::IndexMap;
    use crate::extractor::TResEntryIndexMap;

    #[tokio::test]
    async fn test_func() -> Result<()>{
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();

        let resource_fetcher:ResourcesFetcher  = ResourcesFetcher::new();
        let mut manifest_data: TManifestEntryIndexMap = IndexMap::new();

        for region in app_config.available_regions() {
            let patch_manifest_path: PathBuf = Path::new(&app_config.file_path.manifest_output_dir.clone())
                .join(&region)
                .join(&app_config.file_name.patch_manifest_file.clone());

            if let Some(patch_manifest) = FileHandler::read_json::<TResEntryIndexMap>(&patch_manifest_path) {
                manifest_data.insert(region.to_string(), patch_manifest);
                break;
            } else {
                log::warn!("[test] skip region '{}': manifest not found", region);
            }
        }

        resource_fetcher.fetch_and_save_from_manifest(&manifest_data).await?;

        Ok(())
    }
}
