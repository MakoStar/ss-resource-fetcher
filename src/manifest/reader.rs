use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::error::Result;
use crate::model::Region;

const BIN_DIFF_PATCH_INFO: &str = "BIN_DIFF_PATCH_INFO";
const BIN_DIFF_PATCH_VER: &str = "BIN_DIFF_PATCH_VER";
const CLEAR_EXPIRED_FILES: &str = "CLEAR_EXPIRED_FILES";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ManiResource {
    /// 资源文件名
    pub file: String,
    /// 文件校验值
    pub hash: String,
    /// 文件大小
    pub size: u64,
    /// 文件类型
    pub file_type: String,
}

#[derive(Debug, Clone)]
pub struct ManiPatchInfo {
    /// 补丁波次序号
    pub patch_version: u64,
    /// 该波次资源版本
    pub resource_version: String,
    /// 该波次补丁文件数
    pub file_count: u64,
    /// 该波次补丁总大小
    pub patch_size: u64,
}

#[derive(Debug)]
pub struct ManiReader {
    /// 清单文件路径
    mani_file_path: PathBuf,
    /// 资源条目
    resource_data: HashMap<String, ManiResource>,
    /// 配置项
    config_data: HashMap<String, String>,
    /// 补丁波次信息
    patch_infos: Vec<ManiPatchInfo>,
}

impl ManiReader {
    pub fn new<P: AsRef<Path>>(main_file_path: P, region: Option<&Region>) -> Result<Self> {
        let path = main_file_path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read mani file: {}", path.display()))?;

        let mut resource_data = HashMap::new();
        let mut config_data = HashMap::new();
        let mut patch_infos = Vec::new();

        for raw_line in content.lines() {
            let line: &str = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            match line.strip_prefix('$') {
                Some(entry) => {
                    if let Some((key, value)) = entry.split_once(':') {
                        let key = key.trim();
                        let value = value.trim();

                        if key == BIN_DIFF_PATCH_INFO {
                            if let Some(info) = Self::parse_patch_info(value) {
                                patch_infos.push(info);
                            }
                        }

                        config_data.insert(key.to_string(), value.to_string());
                    }
                }
                None => {
                    let parts: Vec<_> = line.split('|').collect();
                    if parts.len() < 4 {
                        log::debug!("Skipping malformed resource line: {line}");
                        continue;
                    }

                    let file_name = parts[0].trim().to_string();
                    resource_data.insert(
                        file_name.clone(),
                        ManiResource {
                            file: file_name,
                            hash: parts[1].trim().to_string(),
                            size: parts[2].trim().parse().unwrap_or(0),
                            file_type: parts[3].trim().to_string(),
                        },
                    );
                }
            }
        }

        patch_infos.sort_by_key(|info| info.patch_version);

        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();

        match region {
            Some(region) => log::debug!(
                "[{region}] parsed {name} resources={} configs={}",
                resource_data.len(),
                config_data.len()
            ),
            None => log::debug!(
                "parsed {name} resources={} configs={}",
                resource_data.len(),
                config_data.len()
            ),
        }

        Ok(Self {
            mani_file_path: path,
            resource_data,
            config_data,
            patch_infos,
        })
    }

    fn parse_patch_info(value: &str) -> Option<ManiPatchInfo> {
        let parts: Vec<_> = value.split('|').collect();
        if parts.len() < 4 {
            log::debug!("Skipping malformed patch info: {value}");
            return None;
        }

        Some(ManiPatchInfo {
            patch_version: parts[0].trim().parse().unwrap_or(0),
            resource_version: parts[1].trim().to_string(),
            file_count: parts[2].trim().parse().unwrap_or(0),
            patch_size: parts[3].trim().parse().unwrap_or(0),
        })
    }

    pub fn patch_infos(&self) -> &[ManiPatchInfo] {
        &self.patch_infos
    }

    pub fn bin_diff_patch_version(&self) -> Option<u64> {
        self.config_data
            .get(BIN_DIFF_PATCH_VER)
            .and_then(|value| value.parse().ok())
            .or_else(|| self.patch_infos.last().map(|info| info.patch_version))
    }

    pub fn clear_expired_files(&self) -> bool {
        self.config_data
            .get(CLEAR_EXPIRED_FILES)
            .is_some_and(|value| value == "1")
    }

    pub fn resources(&self) -> Vec<&ManiResource> {
        let mut items: Vec<_> = self.resource_data.values().collect();
        items.sort_by(|a, b| a.file.cmp(&b.file));

        items
    }

    pub fn get_resource_hash_by_patch(&self, patch_name: &str) -> Option<&str> {
        if self.resource_data.is_empty() {
            log::warn!(
                "manifest resource data is empty for {:?}",
                self.mani_file_path
            );
            return None;
        }

        self.resource_data
            .get(patch_name)
            .map(|res| res.hash.as_str())
    }

    pub fn get_config_value_by_key(&self, key: &str) -> Option<&str> {
        if self.config_data.is_empty() {
            log::warn!(
                "{} - manifest config data is empty for {:?}",
                key,
                self.mani_file_path
            );
            return None;
        }

        match self.config_data.get(key) {
            Some(value) => Some(value.as_str()),
            None => {
                log::warn!(
                    "available config keys: {:?}",
                    self.config_data.keys().collect::<Vec<_>>()
                );
                log::warn!("config key '{}' not found", key);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ManiReader;

    const SAMPLE: &str = "\
        $CLIENT_VER:1.2.3
        $GAME_VER:4.5.6
        $BIN_DIFF_PATCH_VER:12
        $BIN_DIFF_PATCH_INFO:2|2607301222|43|51460293
        $BIN_DIFF_PATCH_INFO:1|2607231305|20|10309888
        $CLEAR_EXPIRED_FILES:1

        lua.arcx|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|1024|4201
        data.arcx|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|2048|8402
        ";

    fn write_sample(content: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(file.path(), content).expect("failed to write temp file");
        file
    }

    #[test]
    fn parses_resources_and_configs() {
        let file = write_sample(SAMPLE);
        let reader = ManiReader::new(file.path(), None).expect("failed to parse");

        assert_eq!(
            reader.get_resource_hash_by_patch("lua.arcx"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            reader.get_resource_hash_by_patch("data.arcx"),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert_eq!(reader.get_resource_hash_by_patch("missing.arcx"), None);

        assert_eq!(reader.get_config_value_by_key("CLIENT_VER"), Some("1.2.3"));
        assert_eq!(reader.get_config_value_by_key("GAME_VER"), Some("4.5.6"));
        assert_eq!(
            reader.get_config_value_by_key("BIN_DIFF_PATCH_VER"),
            Some("12")
        );
        assert_eq!(reader.get_config_value_by_key("NOT_EXIST"), None);
    }

    #[test]
    fn parses_every_repeated_patch_info() {
        let file = write_sample(SAMPLE);
        let reader = ManiReader::new(file.path(), None).expect("failed to parse");

        let infos = reader.patch_infos();
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].patch_version, 1);
        assert_eq!(infos[0].resource_version, "2607231305");
        assert_eq!(infos[0].file_count, 20);
        assert_eq!(infos[0].patch_size, 10309888);
        assert_eq!(infos[1].patch_version, 2);
        assert_eq!(reader.bin_diff_patch_version(), Some(12));
        assert!(reader.clear_expired_files());
    }

    #[test]
    fn falls_back_to_last_patch_info_version() {
        let file = write_sample("$BIN_DIFF_PATCH_INFO:3|2608071903|544|198691875\n");
        let reader = ManiReader::new(file.path(), None).expect("failed to parse");

        assert_eq!(reader.bin_diff_patch_version(), Some(3));
        assert!(!reader.clear_expired_files());
    }

    #[test]
    fn reports_missing_file_as_error() {
        let err = ManiReader::new("definitely/not/here.mani", None).unwrap_err();
        assert!(err.to_string().contains("Failed to read mani file"));
    }

    #[test]
    fn tolerates_malformed_lines() {
        let file = write_sample("garbage-line\n$NO_COLON_HERE\nlua.arcx|hash|size\n");
        let reader = ManiReader::new(file.path(), None).expect("failed to parse");

        assert!(reader.get_resource_hash_by_patch("lua.arcx").is_none());
        assert!(reader.get_config_value_by_key("CLIENT_VER").is_none());
    }

    const REAL_MANI: &str = include_str!("../../tests/fixtures/ss_win.mani");

    #[test]
    fn parses_real_manifest_configs() {
        let file = write_sample(REAL_MANI);
        let reader = ManiReader::new(file.path(), None).expect("real manifest should parse");

        assert_eq!(reader.get_config_value_by_key("CLIENT_VER"), Some("1.13.0"));
        assert_eq!(reader.get_config_value_by_key("GAME_VER"), Some("1.16.0"));
        assert_eq!(
            reader.get_config_value_by_key("CLIENT_VER_COMP_MODE"),
            Some("TARGET")
        );
        assert_eq!(
            reader.get_config_value_by_key("CLEAR_EXPIRED_FILES"),
            Some("1")
        );
        assert!(reader.clear_expired_files());
    }

    #[test]
    fn parses_real_manifest_resources() {
        let file = write_sample(REAL_MANI);
        let reader = ManiReader::new(file.path(), None).expect("real manifest should parse");

        let hash = reader
            .get_resource_hash_by_patch("char_2d_10801.unity3d")
            .expect("known resource should be present");
        assert_eq!(hash.len(), 32, "md5 hash should be 32 hex chars");

        assert!(
            reader
                .get_resource_hash_by_patch("no_such_resource.unity3d")
                .is_none()
        );
    }

    #[test]
    fn real_manifest_has_expected_resource_count() {
        let file = write_sample(REAL_MANI);
        let reader = ManiReader::new(file.path(), None).expect("real manifest should parse");

        let names: Vec<&str> = REAL_MANI
            .lines()
            .filter(|line| line.contains('|') && !line.starts_with('$'))
            .map(|line| line.split('|').next().unwrap_or_default())
            .collect();

        assert_eq!(names.len(), 124);
        assert!(names.iter().all(|name| name.ends_with(".unity3d")));
        for name in &names {
            assert!(
                reader.get_resource_hash_by_patch(name).is_some(),
                "{name} should be indexed"
            );
        }

        assert!(reader.bin_diff_patch_version().is_none());
    }
}
