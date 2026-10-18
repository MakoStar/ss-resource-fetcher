use std::path::{Path, PathBuf};

use file_utils::FileHandler;
use indexmap::IndexMap;
use network_manager::HttpFetcher;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::model::{DownloadProgress, Region, ResourceEntry, tier_label};
use crate::storage::push_platform;

#[derive(Debug, Default, Clone, Copy)]
pub struct FetchStats {
    /// 实际下载的文件数
    pub downloaded: usize,
    /// 命中缓存的文件数
    pub cached: usize,
    /// 下载字节总数
    pub bytes: u64,
}

impl FetchStats {
    pub fn total(&self) -> usize {
        self.downloaded + self.cached
    }

    pub fn megabytes(&self) -> f64 {
        self.bytes as f64 / 1024.0 / 1024.0
    }

    pub fn add(&mut self, other: &Self) {
        self.downloaded += other.downloaded;
        self.cached += other.cached;
        self.bytes += other.bytes;
    }
}

pub struct ResourcesFetcher {
    /// 资源接口路径
    resource_route: String,
    /// 输出目录
    output_dir: PathBuf,
    /// 平台子目录 空则不加
    platform: String,
    /// 各区域服务器地址
    servers: IndexMap<Region, String>,
    /// 是否覆盖已存在的文件
    overwrite: bool,
    /// 请求客户端
    requester: HttpFetcher,
}

impl ResourcesFetcher {
    pub fn new(
        resource_route: String,
        output_dir: impl Into<PathBuf>,
        platform: impl Into<String>,
        servers: IndexMap<Region, String>,
        overwrite: bool,
        requester: HttpFetcher,
    ) -> Self {
        Self {
            resource_route,
            output_dir: output_dir.into(),
            platform: platform.into(),
            servers,
            overwrite,
            requester,
        }
    }

    pub fn from_app_config(app: &AppConfig) -> Self {
        Self::new(
            app.task.client.route.resource.clone(),
            app.task.client.filepath.patch.clone(),
            app.global.default_platform.clone(),
            app.server_urls(),
            app.task.client.flags.op.overwrite_resource,
            HttpFetcher::new(&app.build_fetcher_config()),
        )
    }

    pub async fn fetch_group(
        &self,
        region: &Region,
        tier: Option<u64>,
        group: &[(&str, &ResourceEntry)],
    ) -> Result<FetchStats> {
        let server_url = self
            .servers
            .get(region)
            .ok_or_else(|| AppError::MissingServer(region.to_string()))?
            .as_str();

        let total = group.len();
        let mut stats = FetchStats::default();

        if let Some(tier) = tier {
            logger::tier!("{} - {} file(s)", tier_label(tier), total);
        }

        for (index, (resource_name, entry)) in group.iter().enumerate() {
            let size = self
                .fetch_and_save_one(
                    region,
                    server_url,
                    resource_name,
                    entry,
                    DownloadProgress::new(index + 1, total),
                )
                .await?;

            if size == 0 {
                stats.cached += 1;
            } else {
                stats.downloaded += 1;
                stats.bytes += size as u64;
            }
        }

        Ok(stats)
    }

    async fn fetch_and_save_one(
        &self,
        region: &Region,
        server_url: &str,
        resource_name: &str,
        entry: &ResourceEntry,
        progress: DownloadProgress,
    ) -> Result<usize> {
        let save_path = self.output_path(region, resource_name);
        let prefix = progress.prefix();

        if !self.overwrite && self.is_cached(&save_path, &entry.hash) {
            logger::step!("[{region}] {prefix} cached: {resource_name}");
            return Ok(0);
        }

        logger::step!("[{region}] {prefix} fetch: {resource_name}");

        let url = self.build_url(
            server_url,
            entry.version,
            &entry.additional_path,
            &entry.file_name,
        );
        let data = self.download(region, &url, resource_name).await?;

        FileHandler::write_bytes(&data, &save_path)?;
        FileHandler::verify_md5(&save_path, &entry.hash)?;

        Ok(data.len())
    }

    fn build_url(
        &self,
        server_url: &str,
        version: u64,
        additional_path: &str,
        file_name: &str,
    ) -> String {
        let version = version.to_string();
        [
            server_url.trim_end_matches('/'),
            self.resource_route.trim_matches('/'),
            version.as_str(),
            additional_path.trim_matches('/'),
            file_name.trim_start_matches('/'),
        ]
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
    }

    fn output_path(&self, region: &Region, resource_name: &str) -> PathBuf {
        self.region_dir(region).join(resource_name)
    }

    fn region_dir(&self, region: &Region) -> PathBuf {
        push_platform(self.output_dir.join(region.as_str()), &self.platform)
    }

    fn is_cached(&self, save_path: &Path, expected_hash: &str) -> bool {
        if !save_path.is_file() {
            return false;
        }

        FileHandler::verify_md5(save_path, expected_hash).is_ok()
    }

    async fn download(&self, region: &Region, url: &str, resource_name: &str) -> Result<Vec<u8>> {
        let bytes = self
            .requester
            .get_bytes(url)
            .await
            .inspect_err(|err| log::error!("[{region}] fetch failed: {resource_name} {err}"))?;

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use network_manager::HttpFetcher;

    use super::{FetchStats, ResourcesFetcher};
    use crate::config::AppConfig;
    use crate::model::Region;

    fn fetcher(route: &str, platform: &str) -> ResourcesFetcher {
        let servers: IndexMap<Region, String> = [
            (
                Region::new("CN"),
                "https://nova-static.yostar.cn/".to_string(),
            ),
            (
                Region::new("EN"),
                "https://nova-static.stellasora.global".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        ResourcesFetcher::new(
            route.to_string(),
            "out/Metadata",
            platform.to_string(),
            servers,
            false,
            HttpFetcher::with_defaults(),
        )
    }

    #[test]
    fn build_url_joins_every_segment() {
        let fetcher = fetcher("/res/", "win");

        let url = fetcher.build_url("https://host/", 133, "meta", "win.html");

        assert_eq!(url, "https://host/res/133/meta/win.html");
    }

    #[test]
    fn build_url_trims_stray_slashes() {
        let fetcher = fetcher("res", "win");

        let url = fetcher.build_url("https://host//", 133, "/meta/", "/win.html");

        assert_eq!(url, "https://host/res/133/meta/win.html");
    }

    #[test]
    fn build_url_drops_empty_additional_path() {
        let fetcher = fetcher("res", "win");

        let url = fetcher.build_url("https://host", 133, "", "data.arcx");

        assert_eq!(url, "https://host/res/133/data.arcx");
    }

    #[test]
    fn build_url_keeps_nested_additional_path() {
        let fetcher = fetcher("res", "win");

        let url = fetcher.build_url("https://host", 1, "a/b/c", "d.bin");

        assert_eq!(url, "https://host/res/1/a/b/c/d.bin");
    }

    #[test]
    fn output_path_carries_region_and_platform() {
        let fetcher = fetcher("res", "win");

        let path = fetcher.output_path(&Region::new("CN"), "p_1_u.data.arcx");

        assert_eq!(
            path,
            std::path::PathBuf::from("out/Metadata/CN/win/p_1_u.data.arcx")
        );
    }

    #[test]
    fn output_path_omits_platform_when_empty() {
        let fetcher = fetcher("res", "");

        let path = fetcher.output_path(&Region::new("EN"), "data.arcx");

        assert_eq!(path, std::path::PathBuf::from("out/Metadata/EN/data.arcx"));
    }

    #[test]
    fn is_cached_compares_md5() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("res.bin");
        std::fs::write(&path, b"abc").unwrap();

        let fetcher = fetcher("res", "win");

        assert!(fetcher.is_cached(&path, "900150983CD24FB0D6963F7D28E17F72"));
        assert!(!fetcher.is_cached(&path, &"0".repeat(32)));
        assert!(!fetcher.is_cached(&dir.path().join("missing"), &"0".repeat(32)));
    }

    #[tokio::test]
    async fn fetch_group_rejects_unknown_region() {
        let fetcher = fetcher("res", "win");

        let err = fetcher
            .fetch_group(&Region::new("TW"), None, &[])
            .await
            .unwrap_err();

        assert!(err.to_string().contains("TW"), "{err}");
    }

    #[tokio::test]
    async fn fetch_group_with_empty_list_reports_nothing() {
        let fetcher = fetcher("res", "win");

        let stats = fetcher
            .fetch_group(&Region::new("CN"), None, &[])
            .await
            .unwrap();

        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn fetch_stats_totals_and_megabytes() {
        let stats = FetchStats {
            downloaded: 3,
            cached: 7,
            bytes: 2 * 1024 * 1024,
        };

        assert_eq!(stats.total(), 10);
        assert!((stats.megabytes() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fetch_stats_add_accumulates() {
        let mut base = FetchStats {
            downloaded: 1,
            cached: 2,
            bytes: 1024,
        };
        base.add(&FetchStats {
            downloaded: 4,
            cached: 5,
            bytes: 2048,
        });

        assert_eq!(base.total(), 12);
        assert_eq!(base.bytes, 3072);
    }

    #[test]
    fn from_app_config_reads_platform_and_servers() {
        let app = AppConfig::default();
        let fetcher = ResourcesFetcher::from_app_config(&app);

        let path = fetcher.output_path(&Region::new("CN"), "x.arcx");
        assert!(
            path.to_string_lossy().contains("win"),
            "default platform layer missing: {path:?}"
        );
    }
}
