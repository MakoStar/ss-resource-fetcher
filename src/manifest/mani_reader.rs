use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::error::Result;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ManiResource {
    pub file: String,
    pub hash: String,
    pub size: u64,
    pub file_type: String,
}

#[derive(Debug)]
pub struct ManiReader {
    mani_file_path: PathBuf,
    resource_data: HashMap<String, ManiResource>,
    config_data: HashMap<String, String>,
}

impl ManiReader {
    pub fn new<P: AsRef<Path>>(main_file_path: P) -> Result<Self> {
        let path: PathBuf= main_file_path.as_ref().to_path_buf();
        let content: String = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read mani file: {}", path.display()))?;

        let mut resource_data: HashMap<String, ManiResource> = HashMap::new();
        let mut config_data: HashMap<String, String> = HashMap::new();

        for raw_line in content.lines() {
            let line: &str = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            match line.strip_prefix('$') {
                Some(entry) => {
                    if let Some((key, value)) = entry.split_once(':') {
                        config_data.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
                None => {
                    let parts: Vec<&str> = line.split('|').collect();
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

        log::debug!(
            "parsed mani: resources={}, configs={} | {}",
            resource_data.len(),
            config_data.len(),
            path.display()
        );

        Ok(Self {
            mani_file_path: path,
            resource_data,
            config_data,
        })
    }

    pub fn resource_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.resource_data.keys().map(String::as_str).collect();
        names.sort_unstable();

        names
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
        let reader = ManiReader::new(file.path()).expect("failed to parse");

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
    fn reports_missing_file_as_error() {
        let err = ManiReader::new("definitely/not/here.mani").unwrap_err();
        assert!(err.to_string().contains("Failed to read mani file"));
    }

    #[test]
    fn tolerates_malformed_lines() {
        let file = write_sample("garbage-line\n$NO_COLON_HERE\nlua.arcx|hash|size\n");
        let reader = ManiReader::new(file.path()).expect("failed to parse");

        assert!(reader.get_resource_hash_by_patch("lua.arcx").is_none());
        assert!(reader.get_config_value_by_key("CLIENT_VER").is_none());
    }
}
