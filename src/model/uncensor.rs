use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use crate::model::Region;

#[derive(Debug, Clone, Deserialize)]
pub struct CustomFileList {
    /// 区域
    pub region: String,
    /// 资源文件名列表
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UncensorSource {
    /// 与官方清单匹配后下载 区域取配置里的来源区域
    #[default]
    ConfigRegion,
    /// 与官方清单匹配后下载 区域由命令行指定
    Region(Region),
    /// 直接用默认地址上的同名资源下载
    DefaultUrl,
    /// 使用自定义文件列表
    CustomFiles {
        /// 自定义文件列表路径
        path: PathBuf,
        /// 可选的区域覆盖 未指定时用文件列表里的区域
        region: Option<Region>,
    },
}

impl UncensorSource {
    pub fn resolve(
        region: Option<String>,
        default_url: bool,
        custom_files: bool,
        custom_files_path: Option<PathBuf>,
    ) -> Result<Self> {
        if custom_files {
            let Some(path) = custom_files_path else {
                anyhow::bail!("--uncensor-custom-files requires --uncensor-custom-files-path");
            };
            return Ok(Self::CustomFiles {
                path,
                region: normalize_region(region),
            });
        }

        if default_url {
            return Ok(Self::DefaultUrl);
        }

        Ok(match normalize_region(region) {
            Some(region) => Self::Region(region),
            None => Self::ConfigRegion,
        })
    }
}

fn normalize_region(region: Option<String>) -> Option<Region> {
    region
        .filter(|value| !value.trim().is_empty())
        .map(|value| Region::new(value.trim().to_uppercase()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::UncensorSource;
    use crate::model::Region;

    #[test]
    fn default_source_is_config_region() {
        assert_eq!(UncensorSource::default(), UncensorSource::ConfigRegion);
        assert_eq!(
            UncensorSource::resolve(None, false, false, None).unwrap(),
            UncensorSource::ConfigRegion
        );
    }

    #[test]
    fn region_arg_overrides_config() {
        assert_eq!(
            UncensorSource::resolve(Some("tw".into()), false, false, None).unwrap(),
            UncensorSource::Region(Region::new("TW"))
        );
    }

    #[test]
    fn default_url_wins_over_region() {
        assert_eq!(
            UncensorSource::resolve(Some("TW".into()), true, false, None).unwrap(),
            UncensorSource::DefaultUrl
        );
    }

    #[test]
    fn blank_region_falls_back_to_config() {
        assert_eq!(
            UncensorSource::resolve(Some("  ".into()), false, false, None).unwrap(),
            UncensorSource::ConfigRegion
        );
    }

    #[test]
    fn custom_files_requires_path() {
        assert!(UncensorSource::resolve(None, false, true, None).is_err());
    }

    #[test]
    fn custom_files_with_region_override() {
        let source = UncensorSource::resolve(
            Some("jp".into()),
            false,
            true,
            Some(PathBuf::from("list.json")),
        )
        .unwrap();

        assert_eq!(
            source,
            UncensorSource::CustomFiles {
                path: PathBuf::from("list.json"),
                region: Some(Region::new("JP")),
            }
        );
    }

    #[test]
    fn custom_files_without_region_override() {
        let source =
            UncensorSource::resolve(None, false, true, Some(PathBuf::from("list.json"))).unwrap();

        assert_eq!(
            source,
            UncensorSource::CustomFiles {
                path: PathBuf::from("list.json"),
                region: None,
            }
        );
    }
}
