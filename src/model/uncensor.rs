use crate::model::Region;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UncensorSource {
    /// 区域取 `[UNCENSOR].SOURCE_REGION`
    #[default]
    ConfigRegion,
    /// 区域由命令行 `--uncensor-region` 指定
    Region(Region),
    /// 不经过官方清单
    DefaultUrl,
}

impl UncensorSource {
    pub fn resolve(region: Option<String>, default_url: bool) -> Self {
        if default_url {
            return Self::DefaultUrl;
        }

        match region {
            Some(region) if !region.trim().is_empty() => {
                Self::Region(Region::new(region.trim().to_uppercase()))
            }
            _ => Self::ConfigRegion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UncensorSource;
    use crate::model::Region;

    #[test]
    fn default_source_is_config_region() {
        assert_eq!(UncensorSource::default(), UncensorSource::ConfigRegion);
        assert_eq!(
            UncensorSource::resolve(None, false),
            UncensorSource::ConfigRegion
        );
    }

    #[test]
    fn region_arg_overrides_config() {
        assert_eq!(
            UncensorSource::resolve(Some("tw".into()), false),
            UncensorSource::Region(Region::new("TW"))
        );
    }

    #[test]
    fn default_url_wins_over_region() {
        assert_eq!(
            UncensorSource::resolve(Some("TW".into()), true),
            UncensorSource::DefaultUrl
        );
    }

    #[test]
    fn blank_region_falls_back_to_config() {
        assert_eq!(
            UncensorSource::resolve(Some("  ".into()), false),
            UncensorSource::ConfigRegion
        );
    }
}
