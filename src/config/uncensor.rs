use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UncensorConfig {
    /// Uncensor 清单所在服务器
    #[serde(rename = "URL")]
    pub url: String,

    /// 清单在该服务器上的路径
    #[serde(rename = "MANIFEST_ROUTE")]
    pub manifest_route: String,

    /// 实际下载资源时使用的官方区域
    #[serde(rename = "SOURCE_REGION")]
    pub source_region: String,
}

impl Default for UncensorConfig {
    fn default() -> Self {
        Self {
            url: "http://na.jvav.net.cn".into(),
            manifest_route: "res/win/ss_win.mani".into(),
            source_region: "TW".into(),
        }
    }
}

impl UncensorConfig {
    pub fn manifest_url(&self) -> String {
        format!(
            "{}/{}",
            self.url.trim_end_matches('/'),
            self.manifest_route.trim_start_matches('/'),
        )
    }

    pub fn resource_base_url(&self) -> String {
        let base = self.url.trim_end_matches('/');
        let route = self.manifest_route.trim_start_matches('/');

        match route.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => format!("{base}/{dir}"),
            _ => base.to_string(),
        }
    }

    pub fn manifest_file_name(&self) -> &str {
        self.manifest_route
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("ss_win.mani")
    }
}

#[cfg(test)]
mod tests {
    use super::UncensorConfig;

    #[test]
    fn builds_default_uncensor_urls() {
        let config = UncensorConfig::default();

        assert_eq!(
            config.manifest_url(),
            "http://na.jvav.net.cn/res/win/ss_win.mani"
        );
        assert_eq!(config.resource_base_url(), "http://na.jvav.net.cn/res/win");
    }

    #[test]
    fn resource_base_url_points_at_manifest_dir() {
        let config = UncensorConfig {
            url: "https://example.com/".into(),
            manifest_route: "/a/b/c/other.mani".into(),
            source_region: "TW".into(),
        };

        assert_eq!(
            config.manifest_url(),
            "https://example.com/a/b/c/other.mani"
        );
        assert_eq!(config.resource_base_url(), "https://example.com/a/b/c");
    }

    #[test]
    fn resource_base_url_falls_back_to_root() {
        let config = UncensorConfig {
            url: "https://example.com".into(),
            manifest_route: "ss_win.mani".into(),
            source_region: "TW".into(),
        };

        assert_eq!(config.resource_base_url(), "https://example.com");
    }

    #[test]
    fn manifest_file_name_takes_last_route_segment() {
        let config = UncensorConfig::default();
        assert_eq!(config.manifest_file_name(), "ss_win.mani");

        let config = UncensorConfig {
            manifest_route: "/res/win/other.mani".into(),
            ..Default::default()
        };
        assert_eq!(config.manifest_file_name(), "other.mani");

        let config = UncensorConfig {
            manifest_route: String::new(),
            ..Default::default()
        };
        assert_eq!(config.manifest_file_name(), "ss_win.mani");
    }
}
