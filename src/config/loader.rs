use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use anyhow::{Context, Result};

use crate::config::{AppConfig, apply_overrides};

pub const CONFIG_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

pub static APP_CONFIG: LazyLock<Result<AppConfig>> =
    LazyLock::new(|| load_or_init_config(CONFIG_FILE_NAME));

static CONFIG_COMMENTS: AtomicBool = AtomicBool::new(false);
static DEFAULT_REGION_FILTERED: AtomicBool = AtomicBool::new(false);
static CONFIG_INIT: AtomicU8 = AtomicU8::new(INIT_AUTO);

const INIT_AUTO: u8 = 0;
const INIT_FORCE: u8 = 1;
const INIT_NEVER: u8 = 2;

pub fn set_config_comments(enabled: bool) {
    CONFIG_COMMENTS.store(enabled, Ordering::Relaxed);
}

pub fn config_comments_enabled() -> bool {
    CONFIG_COMMENTS.load(Ordering::Relaxed)
}

pub fn set_config_init(mode: Option<bool>) {
    let flag: u8 = match mode {
        None => INIT_AUTO,
        Some(true) => INIT_FORCE,
        Some(false) => INIT_NEVER,
    };

    CONFIG_INIT.store(flag, Ordering::Relaxed);
}

fn config_init_mode() -> u8 {
    CONFIG_INIT.load(Ordering::Relaxed)
}

pub fn default_region_filtered() -> bool {
    DEFAULT_REGION_FILTERED.load(Ordering::Relaxed)
}

fn apply_default_region_filter(cfg: &mut AppConfig) {
    if !cfg.feature_flags.is_use_default_region {
        return;
    }

    let default_region: String = cfg.default_region.clone();
    cfg.servers.retain(|region, _| region == &default_region);
    cfg.launcher
        .servers
        .retain(|region, _| region == &default_region);
    DEFAULT_REGION_FILTERED.store(true, Ordering::Relaxed);
}

pub fn load_or_init_config<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    let path: &Path = path.as_ref();
    let exists: bool = path.exists();
    let mode: u8 = config_init_mode();
    let force: bool = mode == INIT_FORCE;
    let save: bool = mode != INIT_NEVER && (!exists || force);

    let mut cfg: AppConfig = if exists && !force {
        load_config(path)?
    } else {
        if exists {
            logger::tips!("--init regenerates the config file, the existing one is overwritten.");
        } else if save {
            logger::tips!("APPLICATION CONFIGURATION FILE NOT FOUND, GENERATED DEFAULT CONFIG.");
        } else {
            logger::tips!("APPLICATION CONFIGURATION FILE NOT FOUND, USING BUILT-IN DEFAULTS.");
        }

        AppConfig::default()
    };

    cfg = apply_overrides(&cfg)?;

    if save {
        cfg.save(path)?;
    }

    apply_default_region_filter(&mut cfg);
    cfg.validate()?;

    Ok(cfg)
}

fn load_config(path: &Path) -> Result<AppConfig> {
    if config_comments_enabled() {
        logger::tips!("--comments only applies when the config file is generated.");
    }

    let content: String = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let cfg: AppConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}, please check syntax", path.display()))?;

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
