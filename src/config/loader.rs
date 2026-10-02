use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};

use crate::config::AppConfig;

pub const CONFIG_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

pub static APP_CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| load_or_init_config(CONFIG_FILE_NAME).expect("failed to load config"));

fn apply_default_region_filter(cfg: &mut AppConfig) {
    if cfg.feature_flags.is_use_default_region {
        let default_region: String = cfg.default_region.clone();
        cfg.servers.retain(|region, _| region == &default_region);
        logger::tips!(
            "IS_USE_DEFAULT_REGION is enabled, only '{}' region is kept !!!",
            cfg.default_region
        );
    }
}

pub fn load_or_init_config<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    let path: &Path = path.as_ref();

    if !path.exists() {
        let mut cfg: AppConfig = AppConfig::default();
        cfg.save(path)?;
        logger::tips!("APPLICATION CONFIGURATION FILE NOT FOUND, GENERATED DEFAULT CONFIG.");
        apply_default_region_filter(&mut cfg);
        return Ok(cfg);
    }

    let content: String = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut cfg: AppConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}, please check syntax", path.display()))?;

    apply_default_region_filter(&mut cfg);
    cfg.validate()?;

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::load_or_init_config;
    use crate::config::AppConfig;

    #[test]
    fn generates_default_file_when_missing() {
        let temp_dir: std::path::PathBuf = std::env::temp_dir().join("config_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_path: std::path::PathBuf = temp_dir.join("config.toml");

        let _ = std::fs::remove_file(&config_path);
        let _: AppConfig = load_or_init_config(&config_path).unwrap();

        assert!(config_path.exists(), "Config file was not created!");

        let content: String = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.is_empty(), "Config file is empty!");

        logger::test!(
            "Config generated successfully at: {}",
            config_path.display()
        );

        log::info!("Content:\n{}", content);

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
