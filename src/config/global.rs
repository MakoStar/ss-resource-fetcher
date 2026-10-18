use serde::{Deserialize, Serialize};

pub const PLATFORMS: &[&str] = &["win", "and", "ios"];

pub const PLATFORM_PLACEHOLDER: &str = "{platform}";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    /// 默认区域
    #[serde(rename = "DEFAULT_REGION")]
    pub default_region: String,

    /// 默认平台
    #[serde(rename = "DEFAULT_PLATFORM")]
    pub default_platform: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_region: "CN".into(),
            default_platform: "win".into(),
        }
    }
}

pub fn resolve_platform_value(stored: &str, template: &str, platform: &str) -> String {
    let current = template.replace(PLATFORM_PLACEHOLDER, platform);

    if stored.contains(PLATFORM_PLACEHOLDER) {
        return stored.replace(PLATFORM_PLACEHOLDER, platform);
    }

    if stored == current || stored == template {
        return current;
    }

    for other in PLATFORMS {
        if template.replace(PLATFORM_PLACEHOLDER, other) == stored {
            return current;
        }
    }

    stored.to_string()
}

pub fn resolve_platform_list(
    stored: &[String],
    templates: &[String],
    platform: &str,
) -> Vec<String> {
    stored
        .iter()
        .enumerate()
        .map(|(index, item)| match templates.get(index) {
            Some(template) => resolve_platform_value(item, template, platform),
            None => item.clone(),
        })
        .collect()
}

impl GlobalConfig {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn resolve_route(&self, route: &str) -> String {
        route.replace(PLATFORM_PLACEHOLDER, &self.default_platform)
    }
}
