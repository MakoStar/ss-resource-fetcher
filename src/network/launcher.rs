use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use file_utils::FileHandler;
use indexmap::IndexMap;
use md5::{Digest, Md5};
use network_manager::{HttpFetcher, HttpMethod, RequestSpec};
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::config::{AppConfig, LauncherConfig, LauncherServerConfig, compile_patterns};
use crate::error::{AppError, Result};
use crate::model::{DownloadProgress, LauncherSelect, LauncherSource, Region, SEPARATOR};
use crate::storage::RegionFileStore;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36";

pub struct LauncherFetcher {
    /// launcher 端点配置
    settings: LauncherConfig,
    /// 资源输出目录
    output_dir: PathBuf,
    /// 清单文件名
    manifest_file: String,
    /// 是否强制覆盖已存在的文件
    overwrite: bool,
    /// 资源筛选正则
    patterns: Vec<Regex>,
    /// 指定下载的文件名
    files: Vec<String>,
    /// 是否按清单里的 path 建目录保存
    keep_path: bool,
    /// 是否下载清单里的全部资源
    all_files: bool,
    /// 请求客户端
    requester: HttpFetcher,
}

impl LauncherFetcher {
    pub fn from_app_config(app: &AppConfig) -> Result<Self> {
        Self::new(
            app.launcher.clone(),
            PathBuf::from(&app.file_path.launcher_output_dir),
            app.file_name.launcher_manifest_file.clone(),
            app.feature_flags.is_overwrite_resource,
            app.launcher.patterns()?.to_vec(),
            HttpFetcher::new(&app.build_fetcher_config()),
        )
    }

    pub fn new(
        settings: LauncherConfig,
        output_dir: impl Into<PathBuf>,
        manifest_file: impl Into<String>,
        overwrite: bool,
        patterns: Vec<Regex>,
        requester: HttpFetcher,
    ) -> Result<Self> {
        if settings.servers.is_empty() {
            return Err(AppError::message(
                "no launcher endpoint configured: [LAUNCHER.SERVERS] is empty",
            ));
        }

        Ok(Self {
            settings,
            output_dir: output_dir.into(),
            manifest_file: manifest_file.into(),
            overwrite,
            patterns,
            files: Vec::new(),
            keep_path: false,
            all_files: false,
            requester,
        })
    }

    pub fn with_select(mut self, select: &LauncherSelect) -> Result<Self> {
        match select {
            LauncherSelect::All => self.all_files = true,
            LauncherSelect::Regex(patterns) => self.patterns = compile_patterns(patterns)?,
            LauncherSelect::Files(files) => self.files = files.clone(),
        }

        Ok(self)
    }

    pub fn with_keep_path(mut self, keep_path: bool) -> Self {
        self.keep_path = keep_path;
        self
    }

    pub async fn fetch_all(&self, source: &LauncherSource) -> Result<()> {
        for (region, server) in self.servers(source)? {
            self.fetch_region(&region, &server).await?;
        }

        Ok(())
    }

    fn servers(&self, source: &LauncherSource) -> Result<IndexMap<Region, LauncherServerConfig>> {
        let mut servers: IndexMap<Region, LauncherServerConfig> = IndexMap::new();

        for (name, server) in &self.settings.servers {
            let region: Region = Region::new(name.clone());
            if source.matches(&region) {
                servers.insert(region, server.clone());
            }
        }

        if servers.is_empty() {
            if let LauncherSource::Region(region) = source {
                if let Some(server) = self.settings.server(region.as_str()) {
                    servers.insert(region.clone(), server);
                    return Ok(servers);
                }
            }

            match source {
                LauncherSource::Region(region) => {
                    return Err(AppError::message(format!(
                        "launcher region '{region}' has no endpoint. Available: {}",
                        self.settings
                            .builtin_region_names()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
                LauncherSource::AllRegions => {
                    return Err(AppError::message(
                        "no launcher region selected, check [LAUNCHER.SERVERS]",
                    ));
                }
            }
        }

        Ok(servers)
    }

    async fn fetch_region(&self, region: &Region, server: &LauncherServerConfig) -> Result<()> {
        let manifest: LauncherManifest = self.fetch_manifest(region, server).await?;
        self.download_resources(region, server, &manifest).await
    }

    async fn fetch_manifest(
        &self,
        region: &Region,
        server: &LauncherServerConfig,
    ) -> Result<LauncherManifest> {
        logger::head!("FETCH LAUNCHER CONFIG {region}");

        let config_url: String = self.settings.config_url(server);
        let payload: ConfigPayload = self.get_json(&config_url, server).await?;
        let version: String = self.payload_field(region, &payload, &self.settings.version_key)?;
        let file_path: String = self.payload_field(region, &payload, &self.settings.path_key)?;

        log::info!("[{region}] launcher version {version}");

        let link_url: String = self
            .settings
            .manifest_link_url(server, &version, &file_path);
        let link: LinkPayload = self.get_json(&link_url, server).await?;
        let bytes: Vec<u8> = self.get_bytes(&link.data.url, server).await?;

        if self.settings.is_save_manifest {
            RegionFileStore::new(&self.output_dir, &self.manifest_file)
                .save_bytes(region, &bytes)?;
        }

        let manifest: LauncherManifest = serde_json::from_slice(&bytes)?;
        log::info!("[{region}] launcher manifest files={}", manifest.file.len());

        Ok(manifest)
    }

    fn payload_field(&self, region: &Region, payload: &ConfigPayload, key: &str) -> Result<String> {
        match payload.data.get(key).and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
            _ => Err(AppError::message(format!(
                "[{region}] launcher config has no usable '{key}' field"
            ))),
        }
    }

    async fn download_resources(
        &self,
        region: &Region,
        server: &LauncherServerConfig,
        manifest: &LauncherManifest,
    ) -> Result<()> {
        let wanted: Vec<LauncherResource> = self.select_resources(server, manifest);
        if wanted.is_empty() {
            log::warn!("[{region}] {}", self.empty_reason());
            return Ok(());
        }

        logger::head!("FETCHING LAUNCHER RESOURCES {region}");
        if self.all_files {
            logger::tips!("launcher --all is enabled, every manifest file will download");
        }

        let missing: usize = self.missing_files(manifest);
        if missing > 0 {
            log::warn!("[{region}] {missing} listed file(s) not in manifest");
        }
        if self.overwrite {
            logger::tips!("IS_OVERWRITE_RESOURCE is enabled, resources will re-download !!!");
        }

        let total: usize = wanted.len();
        log::info!("[{region}] matched {total} launcher resources");

        let mut failed: usize = 0;
        for (index, resource) in wanted.iter().enumerate() {
            let progress: DownloadProgress = DownloadProgress::new(index + 1, total);
            if self
                .download_one(region, server, resource, progress)
                .await
                .is_err()
            {
                failed += 1;
                log::warn!(
                    "[{region}] {} {} failed, skipped",
                    progress.prefix(),
                    resource.name
                );
            }

            if index + 1 < total {
                log::debug!("{SEPARATOR}");
            }
        }

        if failed > 0 {
            log::warn!("[{region}] launcher done, {failed}/{total} failed");
        }

        logger::tips!(
            "[{region}] launcher pack done, {}/{total} file(s) downloaded",
            total - failed
        );

        Ok(())
    }

    fn select_resources(
        &self,
        server: &LauncherServerConfig,
        manifest: &LauncherManifest,
    ) -> Vec<LauncherResource> {
        let mut resources: Vec<LauncherResource> = Vec::new();

        for file in &manifest.file {
            let name: &str = match file.name() {
                Some(name) if !name.is_empty() => name,
                _ => continue,
            };

            let matched: bool = if self.all_files {
                true
            } else if !self.files.is_empty() {
                self.in_file_list(name, file.path.trim_start_matches('/'))
            } else {
                self.patterns.iter().any(|regex| regex.is_match(name))
            };

            if !matched {
                continue;
            }

            resources.push(LauncherResource {
                name: name.to_string(),
                path: file.relative_path(),
                url: LauncherConfig::resource_url(server, &manifest.source, &file.path),
                size: parse_size(&file.size),
            });
        }

        resources
    }

    fn in_file_list(&self, name: &str, path: &str) -> bool {
        self.files
            .iter()
            .any(|entry| entry_matches(entry, name, path))
    }

    fn missing_files(&self, manifest: &LauncherManifest) -> usize {
        self.files
            .iter()
            .filter(|entry| {
                !manifest.file.iter().any(|file| match file.name() {
                    Some(name) if !name.is_empty() => {
                        entry_matches(entry, name, file.path.trim_start_matches('/'))
                    }
                    _ => false,
                })
            })
            .count()
    }

    fn empty_reason(&self) -> &'static str {
        if self.all_files {
            "launcher manifest has no file"
        } else if self.files.is_empty() {
            "no launcher resource matched the patterns"
        } else {
            "no listed launcher file found"
        }
    }

    async fn download_one(
        &self,
        region: &Region,
        server: &LauncherServerConfig,
        resource: &LauncherResource,
        progress: DownloadProgress,
    ) -> Result<()> {
        let prefix: String = progress.prefix();
        let save_path: PathBuf = self.output_path(region, resource);

        if self.is_cached(region, &save_path, resource, &prefix) {
            return Ok(());
        }

        let bytes: Vec<u8> = self.get_bytes(&resource.url, server).await?;
        FileHandler::write_bytes(&bytes, &save_path)?;

        log::info!(
            "[{region}] {} {} - {:.2}KB",
            prefix,
            resource.name,
            bytes.len() as f64 / 1024.0
        );

        if let Some(expected) = resource.size
            && bytes.len() as u64 != expected
        {
            log::warn!(
                "[{region}] {} {} size mismatch: got {} expected {expected}",
                prefix,
                resource.name,
                bytes.len()
            );
        }

        Ok(())
    }

    fn is_cached(
        &self,
        region: &Region,
        save_path: &Path,
        resource: &LauncherResource,
        prefix: &str,
    ) -> bool {
        if self.overwrite {
            return false;
        }

        if !save_path.is_file() {
            logger::step!(
                "[{region}] {prefix} {} not found locally, will download",
                resource.name
            );
            return false;
        }

        let same_size: bool = match resource.size {
            Some(expected) => FileHandler::file_size(save_path) == Some(expected),
            None => true,
        };

        if same_size {
            logger::step!("[{region}] {prefix} SKIP (cached): {}", resource.name);
            true
        } else {
            log::warn!(
                "[{region}] {prefix} {} size mismatch, will re-download",
                resource.name
            );
            false
        }
    }

    fn output_path(&self, region: &Region, resource: &LauncherResource) -> PathBuf {
        let relative: &str = if self.keep_path {
            &resource.path
        } else {
            &resource.name
        };

        self.output_dir.join(region.as_str()).join(relative)
    }

    fn authorization(&self, server: &LauncherServerConfig) -> Result<String> {
        let head: AuthHead = AuthHead {
            game_tag: &server.tag,
            time: unix_timestamp()?,
            version: &self.settings.version,
        };

        let unsigned: String = serde_json::to_string(&head)?;
        let digest = Md5::digest(format!("{unsigned}{}", server.salt).as_bytes());
        let sign: String = hex::encode(digest);
        let payload: Authorization = Authorization { head, sign };

        Ok(serde_json::to_string(&payload)?)
    }

    async fn get_bytes(&self, url: &str, server: &LauncherServerConfig) -> Result<Vec<u8>> {
        let spec: RequestSpec = RequestSpec::builder(HttpMethod::Get, url)
            .header("User-Agent", USER_AGENT)
            .header("Authorization", self.authorization(server)?)
            .build();

        let response = self
            .requester
            .execute(&spec)
            .await
            .inspect_err(|err| log::error!("url={url} fetch error={err}"))?;

        Ok(response.body)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        server: &LauncherServerConfig,
    ) -> Result<T> {
        let bytes: Vec<u8> = self.get_bytes(url, server).await?;

        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|err| AppError::message(format!("system clock is before unix epoch: {err}")))
}

fn entry_matches(entry: &str, name: &str, path: &str) -> bool {
    let wanted: &str = entry.trim_start_matches('/');

    if wanted.contains('/') {
        wanted == path
    } else {
        wanted == name
    }
}

fn parse_size(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct LauncherResource {
    /// 资源文件名
    name: String,
    /// 清单里的相对路径
    path: String,
    /// 资源完整下载地址
    url: String,
    /// 清单里声明的字节大小
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ConfigPayload {
    #[serde(default)]
    data: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LinkPayload {
    data: LinkData,
}

#[derive(Debug, Deserialize)]
struct LinkData {
    url: String,
}

#[derive(Debug, Deserialize)]
struct LauncherManifest {
    #[serde(default)]
    source: String,
    #[serde(default)]
    file: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    path: String,
    #[serde(default)]
    size: Value,
}

impl ManifestFile {
    fn name(&self) -> Option<&str> {
        self.path.rsplit('/').next()
    }

    fn relative_path(&self) -> String {
        self.path.trim_start_matches('/').to_string()
    }
}

#[derive(Debug, Serialize)]
struct AuthHead<'a> {
    game_tag: &'a str,
    time: u64,
    version: &'a str,
}

#[derive(Debug, Serialize)]
struct Authorization<'a> {
    head: AuthHead<'a>,
    sign: String,
}

#[cfg(test)]
mod tests {
    use super::{LauncherManifest, LauncherResource, ManifestFile, parse_size};
    use crate::config::{LauncherConfig, LauncherServerConfig};
    use crate::model::{LauncherSelect, LauncherSource, Region};
    use serde_json::{Value, json};
    use std::path::Path;

    fn server() -> LauncherServerConfig {
        LauncherServerConfig {
            tag: "StellaSora_KR".into(),
            salt: "salt".into(),
            api_url: "https://api.example.com".into(),
            pkg_url: "https://pkg.example.com".into(),
        }
    }

    fn manifest() -> LauncherManifest {
        LauncherManifest {
            source: "/game-1.0.0".into(),
            file: vec![
                ManifestFile {
                    path: "/StellaSora_Data/data.unity3d".into(),
                    size: json!("1024"),
                },
                ManifestFile {
                    path: "/StellaSora_Data/other.unity3d".into(),
                    size: json!(2048),
                },
            ],
        }
    }

    fn fetcher(select: LauncherSelect) -> super::LauncherFetcher {
        fetcher_with(select, false)
    }

    fn fetcher_with(select: LauncherSelect, keep_path: bool) -> super::LauncherFetcher {
        super::LauncherFetcher::new(
            LauncherConfig::default(),
            "./output/Launcher",
            "launcher_manifest.json",
            false,
            Vec::new(),
            network_manager::HttpFetcher::with_defaults(),
        )
        .unwrap()
        .with_select(&select)
        .unwrap()
        .with_keep_path(keep_path)
    }

    fn regex(patterns: &[&str]) -> LauncherSelect {
        LauncherSelect::Regex(patterns.iter().map(|p| p.to_string()).collect())
    }

    fn files(names: &[&str]) -> LauncherSelect {
        LauncherSelect::Files(names.iter().map(|n| n.to_string()).collect())
    }

    #[test]
    fn selects_only_pattern_matching_files() {
        let selected: Vec<LauncherResource> =
            fetcher(regex(&[r"^data\.unity3d$"])).select_resources(&server(), &manifest());

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "data.unity3d");
        assert_eq!(
            selected[0].url,
            "https://pkg.example.com/game-1.0.0/StellaSora_Data/data.unity3d"
        );
        assert_eq!(selected[0].size, Some(1024));
    }

    #[test]
    fn selects_everything_when_pattern_is_broad() {
        let selected: Vec<LauncherResource> =
            fetcher(regex(&[r"\.unity3d$"])).select_resources(&server(), &manifest());

        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn no_pattern_match_returns_empty() {
        let selected: Vec<LauncherResource> =
            fetcher(regex(&[r"^lua\.arcx$"])).select_resources(&server(), &manifest());

        assert!(selected.is_empty());
    }

    #[test]
    fn file_list_matches_name_and_path() {
        let selected: Vec<LauncherResource> =
            fetcher(files(&["data.unity3d", "StellaSora_Data/other.unity3d"]))
                .select_resources(&server(), &manifest());

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "data.unity3d");
        assert_eq!(selected[1].path, "StellaSora_Data/other.unity3d");
    }

    #[test]
    fn file_list_ignores_unknown_names() {
        let fetcher = fetcher(files(&["data.unity3d", "missing.bin"]));
        let selected: Vec<LauncherResource> = fetcher.select_resources(&server(), &manifest());

        assert_eq!(selected.len(), 1);
        assert_eq!(fetcher.missing_files(&manifest()), 1);
    }

    #[test]
    fn repeated_file_entries_are_not_missing() {
        let fetcher = fetcher(files(&["data.unity3d", "data.unity3d"]));

        assert_eq!(fetcher.missing_files(&manifest()), 0);
    }

    #[test]
    fn all_files_mode_ignores_patterns() {
        let selected: Vec<LauncherResource> =
            fetcher_with(LauncherSelect::All, false).select_resources(&server(), &manifest());

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].path, "StellaSora_Data/data.unity3d");
    }

    #[test]
    fn keep_path_saves_relative_manifest_path() {
        let fetcher = fetcher_with(regex(&[r"^data\.unity3d$"]), true);
        let selected: Vec<LauncherResource> = fetcher.select_resources(&server(), &manifest());

        assert_eq!(
            fetcher.output_path(&Region::new("CN"), &selected[0]),
            Path::new("./output/Launcher/CN/StellaSora_Data/data.unity3d")
        );
    }

    #[test]
    fn flat_mode_saves_file_name_only() {
        let fetcher = fetcher(regex(&[r"^data\.unity3d$"]));
        let selected: Vec<LauncherResource> = fetcher.select_resources(&server(), &manifest());

        assert_eq!(
            fetcher.output_path(&Region::new("CN"), &selected[0]),
            Path::new("./output/Launcher/CN/data.unity3d")
        );
    }

    #[test]
    fn file_name_takes_last_path_segment() {
        let file = ManifestFile {
            path: "/a/b/data.unity3d".into(),
            size: Value::Null,
        };

        assert_eq!(file.name(), Some("data.unity3d"));
        assert_eq!(file.relative_path(), "a/b/data.unity3d");
        assert_eq!(parse_size(&file.size), None);
    }

    #[test]
    fn size_accepts_string_and_number() {
        assert_eq!(parse_size(&json!("4096")), Some(4096));
        assert_eq!(parse_size(&json!(4096)), Some(4096));
        assert_eq!(parse_size(&json!(null)), None);
    }

    #[test]
    fn parses_real_manifest_payload() {
        let payload = r#"{"source":"/StellaSora_KR-1.13.3-game","file":[{"path":"/AntiCheatExpert/ACE-BASE.sys","hash":"11638352398531338391","size":"4282536"}]}"#;
        let parsed: LauncherManifest = serde_json::from_str(payload).unwrap();

        assert_eq!(parsed.source, "/StellaSora_KR-1.13.3-game");
        assert_eq!(parsed.file.len(), 1);
        assert_eq!(parsed.file[0].name(), Some("ACE-BASE.sys"));
        assert_eq!(parse_size(&parsed.file[0].size), Some(4282536));
    }

    #[test]
    fn resource_region_filter_picks_matching_server() {
        let fetcher = fetcher(regex(&[r"^data\.unity3d$"]));
        let source = LauncherSource::Region(Region::new("KR"));

        let servers = fetcher.servers(&source).unwrap();
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key(&Region::new("KR")));
    }

    #[test]
    fn unknown_region_reports_available_ones() {
        let fetcher = fetcher(regex(&[r"^data\.unity3d$"]));
        let source = LauncherSource::Region(Region::new("XX"));

        let err = fetcher.servers(&source).unwrap_err();
        assert!(err.to_string().contains("Available: CN, EN, JP, KR, TW"));
    }

    #[test]
    fn all_regions_keeps_every_configured_endpoint() {
        let fetcher = fetcher(regex(&[r"^data\.unity3d$"]));

        assert_eq!(
            fetcher.servers(&LauncherSource::AllRegions).unwrap().len(),
            5
        );
    }
}
