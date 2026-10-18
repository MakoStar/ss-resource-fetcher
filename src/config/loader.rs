use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use anyhow::{Context, Result};

use crate::config::{AppConfig, CommentLang, apply_overrides};

pub const CONFIG_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

pub static APP_CONFIG: LazyLock<Result<AppConfig>> =
    LazyLock::new(|| load_or_generate(CONFIG_FILE_NAME));

static CONFIG_COMMENTS: AtomicU8 = AtomicU8::new(0);
static CONFIG_GENERATE: AtomicBool = AtomicBool::new(false);
static DEFAULT_REGION_FILTERED: AtomicBool = AtomicBool::new(false);

pub fn set_config_comments(lang: Option<CommentLang>) {
    let value = match lang {
        None => 0,
        Some(CommentLang::En) => 1,
        Some(CommentLang::Cn) => 2,
    };

    CONFIG_COMMENTS.store(value, Ordering::Relaxed);
}

pub fn config_comments_lang() -> Option<CommentLang> {
    match CONFIG_COMMENTS.load(Ordering::Relaxed) {
        1 => Some(CommentLang::En),
        2 => Some(CommentLang::Cn),
        _ => None,
    }
}

pub fn set_config_generate(mode: Option<bool>) {
    CONFIG_GENERATE.store(mode.unwrap_or(false), Ordering::Relaxed);
}

fn config_generate_enabled() -> bool {
    CONFIG_GENERATE.load(Ordering::Relaxed)
}

pub fn default_region_filtered() -> bool {
    DEFAULT_REGION_FILTERED.load(Ordering::Relaxed)
}

fn apply_default_region_filter(cfg: &mut AppConfig) {
    let default_region = cfg.global.default_region.clone();
    let mut filtered = false;

    if cfg.task.client.flags.op.use_default_region {
        cfg.task
            .client
            .servers
            .retain(|region, _| region == &default_region);
        filtered = true;
    }

    if cfg.task.launcher.flags.op.use_default_region {
        cfg.task
            .launcher
            .servers
            .retain(|region, _| region == &default_region);
        filtered = true;
    }

    if filtered {
        DEFAULT_REGION_FILTERED.store(true, Ordering::Relaxed);
    }
}

pub fn load_or_generate<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    generate_config(path.as_ref(), config_generate_enabled())
}

fn generate_config(path: &Path, generate: bool) -> Result<AppConfig> {
    let exists = path.exists();
    let load = exists && !generate;

    let mut cfg = if load {
        load_config(path)?
    } else {
        if generate {
            if exists {
                logger::tips!("--gen_config: regenerating, existing file overwritten");
            } else {
                logger::tips!("--gen_config: default config written");
            }
        } else {
            logger::tips!("config not found, built-in defaults used");
        }

        AppConfig::default()
    };

    cfg = apply_overrides(&cfg)?;

    cfg.apply_platform();

    if generate {
        cfg.save(path)?;
    }

    cfg.validate()?;
    apply_default_region_filter(&mut cfg);

    Ok(cfg)
}

fn load_config(path: &Path) -> Result<AppConfig> {
    if config_comments_lang().is_some() {
        logger::tips!("--add_comments only applies when --gen_config writes the file");
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let cfg = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}, please check syntax", path.display()))?;

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::generate_config;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("config_test");
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);

        path
    }

    #[test]
    fn skips_writing_file_by_default() {
        let path = temp_path("skip.toml");
        let cfg = generate_config(&path, false).unwrap();

        assert!(!path.exists(), "config file must not be written by default");
        assert_eq!(cfg.global.default_region, "CN");
    }

    #[test]
    fn writes_file_when_generate_requested() {
        let path = temp_path("gen.toml");
        let _ = generate_config(&path, true).unwrap();

        assert!(path.exists(), "config file was not created");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "config file is empty");

        logger::test!("config generated at: {}", path.display());
    }

    #[test]
    fn loads_existing_file_without_overwriting() {
        let path = temp_path("load.toml");
        std::fs::write(&path, "[GLOBAL]\nDEFAULT_REGION = \"JP\"\n").unwrap();

        let cfg = generate_config(&path, false).unwrap();
        assert_eq!(cfg.global.default_region, "JP");

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "[GLOBAL]\nDEFAULT_REGION = \"JP\"\n");
    }

    #[test]
    fn generate_overwrites_existing_file() {
        let path = temp_path("overwrite.toml");
        std::fs::write(&path, "[GLOBAL]\nDEFAULT_REGION = \"JP\"\n").unwrap();

        let cfg = generate_config(&path, true).unwrap();
        assert_eq!(cfg.global.default_region, "CN");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("DEFAULT_REGION"));
    }

    #[test]
    fn platform_syncs_server_routes() {
        let path = temp_path("platform.toml");
        std::fs::write(&path, "[GLOBAL]\nDEFAULT_PLATFORM = \"and\"\n").unwrap();

        let cfg = generate_config(&path, false).unwrap();
        assert_eq!(cfg.task.client.route.manifest, "/meta/and.html");
        assert_eq!(cfg.task.client.route.resource, "/res/and/");
        assert_eq!(cfg.task.client.filename.raw_manifest, "and.html");
    }

    #[test]
    fn switching_platform_rewrites_stored_values() {
        let path = temp_path("switch.toml");
        std::fs::write(
                &path,
                "[GLOBAL]\nDEFAULT_PLATFORM = \"and\"\n\n[TASK.CLIENT.FILENAME]\nROOT_MANI = \"ss_win.mani\"\n",
            )
            .unwrap();

        let cfg = generate_config(&path, false).unwrap();
        assert_eq!(cfg.task.client.filename.root_mani, "ss_and.mani");
        assert_eq!(cfg.task.client.route.manifest, "/meta/and.html");
    }

    #[test]
    fn invalid_platform_is_rejected() {
        let path = temp_path("badplat.toml");
        std::fs::write(&path, "[GLOBAL]\nDEFAULT_PLATFORM = \"xbox\"\n").unwrap();

        assert!(generate_config(&path, false).is_err());
    }

    #[test]
    fn invalid_region_lists_available_regions() {
        let path = temp_path("badregion.toml");
        std::fs::write(&path, "[GLOBAL]\nDEFAULT_REGION = \"SS\"\n").unwrap();

        let err = generate_config(&path, false).unwrap_err().to_string();
        assert!(err.contains("DEFAULT_REGION 'SS'"), "{err}");
        assert!(err.contains("CN, EN, JP, KR, TW"), "{err}");
    }

    #[test]
    fn launcher_flag_off_keeps_all_launcher_regions() {
        let path = temp_path("launcher_regions.toml");
        std::fs::write(
            &path,
            "[GLOBAL]\nDEFAULT_REGION = \"CN\"\n\n[TASK.LAUNCHER.FLAGS.OP]\nUSE_DEFAULT_REGION = false\n",
        )
        .unwrap();

        let cfg = generate_config(&path, false).unwrap();

        assert_eq!(cfg.task.client.servers.len(), 1);
        assert!(cfg.task.client.servers.contains_key("CN"));
        assert_eq!(cfg.task.launcher.servers.len(), 5);
        assert!(cfg.task.launcher.servers.contains_key("JP"));
    }
}
