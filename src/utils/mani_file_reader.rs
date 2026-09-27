use std::fs::File;
use std::path::Path;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;
use std::collections::HashMap;

use anyhow::Result;


/// ss_win.mani 文件的资源部分映射表
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ManiResource {
    pub file: String,
    pub hash: String,
    pub size: u64,
    pub file_type: String,
}

/// 清单补丁文件读取器: 针对 ss_win.mani 文件
#[derive(Debug)]
pub struct ManiFileReader {
    mani_file_path: PathBuf,
    resource_data: HashMap<String, ManiResource>,
    config_data: HashMap<String, String>,
}

impl ManiFileReader {
    /// 构造时自动解析资源和配置
    pub fn new<P: AsRef<Path>>(main_file_path: P) -> Result<Self> {
        let path: PathBuf = main_file_path.as_ref().to_path_buf();
        let mut reader: ManiFileReader = Self {
            mani_file_path: path,
            resource_data: HashMap::new(),
            config_data: HashMap::new(),
        };

        reader.parse_resources_data()?;
        reader.parse_configs_data()?;

        Ok(reader)
    }

    // 带错误抑制的行过滤迭代器
    fn filtered_lines_from_path<F>(path: &Path, predicate: F) -> impl Iterator<Item = String>
    where
        F: Fn(&str) -> bool + 'static,
    {
        let file: Option<File> = File::open(path).ok();
        let reader: Option<BufReader<File>> = file.map(BufReader::new);
        reader
            .into_iter()
            .flat_map(|r| r.lines())
            .filter_map(|line| line.ok())
            .map(|line| line.trim().to_string())
            .filter(move |line| !line.is_empty() && predicate(line))
    }

    // 解析文件资源配置项
    fn parse_resources_data(&mut self) -> Result<()> {
        let path: PathBuf = self.mani_file_path.clone();
        let lines = Self::filtered_lines_from_path(&path, |l| !l.starts_with('$'));

        for line in lines {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 4 {
                continue;
            }
            let file_name: String = parts[0].to_string();
            let file_hash: String = parts[1].to_string();
            let size: u64 = parts[2].parse().unwrap_or(0);
            let file_type: String = parts[3].to_string();

            self.resource_data.insert(
                file_name.clone(),
                ManiResource {
                    file: file_name,
                    hash: file_hash,
                    size,
                    file_type,
                },
            );
        }

        Ok(())
    }

    /// 通过补丁文件名获取 hash
    #[allow(dead_code)]
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

    // 解析 $config 配置项
    fn parse_configs_data(&mut self) -> Result<()> {
        let path: PathBuf = self.mani_file_path.clone();
        for line in Self::filtered_lines_from_path(&path, |l| l.starts_with('$')) {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() < 2 {
                continue;
            }
            let key: String = parts[0].trim_start_matches('$').to_string();
            let value: String = parts[1].to_string();
            self.config_data.insert(key, value);
        }

        Ok(())
    }

    /// 通过 config key 获取配置值
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
    use super::*;
    use anyhow::Result;
    use crate::config::AppConfig;

    #[test]
    fn test_func() -> Result<()> {
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();

        let patch_output_dir: String = app_config.file_path.patch_output_dir.clone();
        let root_manifest_patch_name: String = app_config.file_name.update_root_mani_file.clone();
        let read_path: PathBuf = Path::new(&patch_output_dir).join("CN").join(root_manifest_patch_name);
        let reader: ManiFileReader = ManiFileReader::new(read_path)?;

        if let Some(hash) = reader.get_resource_hash_by_patch("lua.arcx") {
            println!("hash: {}", hash);
        } else {
            println!("lua.arcx not found in resource_data");
        }

        if let Some(client_ver) = reader.get_config_value_by_key("CLIENT_VER") {
            println!("client_ver: {}", client_ver);
        } else {
            println!("client_ver not found in config_data");
        }

        if let Some(game_ver) = reader.get_config_value_by_key("GAME_VER") {
            println!("game_ver: {}", game_ver);
        } else {
            println!("game_ver not found in config_data");
        }

        Ok(())
    }
}