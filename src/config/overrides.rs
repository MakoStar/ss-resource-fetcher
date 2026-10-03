use std::collections::HashSet;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow, bail};
use toml::{Table, Value};

use crate::config::AppConfig;

static OVERRIDES: OnceLock<Vec<ConfigOverride>> = OnceLock::new();

const SHORTHAND_SECTIONS: [&str; 7] = [
    "FEATURE_FLAGS",
    "FILE_PATH",
    "FILE_NAME",
    "REQUEST",
    "EXTRACTOR",
    "SERVER_ROUTE",
    "LAUNCHER",
];

#[derive(Debug, Clone)]
pub struct ConfigOverride {
    /// 配置项路径
    pub path: Vec<String>,
    /// 命令行原始输入
    pub raw: String,
    /// 解析后的值
    pub value: Value,
}

pub fn set_config_overrides(items: Vec<ConfigOverride>) {
    let _ = OVERRIDES.set(items);
}

pub fn config_overrides() -> &'static [ConfigOverride] {
    OVERRIDES.get().map(Vec::as_slice).unwrap_or_default()
}

pub fn apply_overrides(cfg: &AppConfig) -> Result<AppConfig> {
    apply(cfg, config_overrides())
}

impl ConfigOverride {
    pub fn parse(raw: &str) -> Result<Self> {
        let (key, value) = raw
            .split_once('=')
            .with_context(|| format!("invalid override '{raw}', expected KEY=VALUE"))?;

        let key: &str = key.trim();
        if key.is_empty() {
            bail!("invalid override '{raw}', key is empty");
        }

        Ok(Self {
            path: key.split('.').map(str::to_string).collect(),
            raw: raw.to_string(),
            value: parse_value(value.trim()),
        })
    }
}

fn apply(cfg: &AppConfig, items: &[ConfigOverride]) -> Result<AppConfig> {
    if items.is_empty() {
        return Ok(cfg.clone());
    }

    let mut root: Table = Table::try_from(cfg).map_err(|err| anyhow!("{err}"))?;
    let mut touched: HashSet<Vec<String>> = HashSet::new();

    for item in items {
        let path: Vec<String> = resolve_path(&root, item)?;
        let append: bool = touched.contains(&path);

        set_value(&mut root, &path, item.value.clone(), append)?;
        touched.insert(path);
        logger::tips!("config override: {}", item.raw);
    }

    root.try_into()
        .map_err(|err| anyhow!("{err}"))
        .context("failed to apply config overrides, check -s KEY=VALUE types")
}

fn resolve_path(root: &Table, item: &ConfigOverride) -> Result<Vec<String>> {
    if item.path.len() > 1 {
        ensure_known(root, &item.path, &item.raw)?;
        return Ok(item.path.clone());
    }

    let key: &str = &item.path[0];
    if root.contains_key(key) {
        return Ok(vec![key.to_string()]);
    }

    for section in SHORTHAND_SECTIONS {
        let Some(Value::Table(table)) = root.get(section) else {
            continue;
        };

        if table.contains_key(key) {
            return Ok(vec![section.to_string(), key.to_string()]);
        }
    }

    bail!("unknown config key '{key}', use SECTION.KEY like FEATURE_FLAGS.IS_MERGE_PATCH")
}

fn ensure_known(root: &Table, path: &[String], raw: &str) -> Result<()> {
    let mut current: &Table = root;

    for (depth, key) in path.iter().enumerate() {
        match current.get(key.as_str()) {
            Some(Value::Table(next)) => current = next,
            Some(_) if depth + 1 == path.len() => {}
            Some(_) => bail!("config key '{raw}': '{key}' is not a section"),
            None => bail!("unknown config key '{raw}': '{key}' not found"),
        }
    }

    Ok(())
}

fn set_value(root: &mut Table, path: &[String], value: Value, append: bool) -> Result<()> {
    let Some((last, parents)) = path.split_last() else {
        return Ok(());
    };

    let mut current: &mut Table = root;
    for key in parents {
        let next: &mut Value = current
            .get_mut(key.as_str())
            .with_context(|| format!("config key '{key}' not found"))?;

        current = next
            .as_table_mut()
            .with_context(|| format!("config key '{key}' is not a section"))?;
    }

    let current_value: Option<&Value> = current.get(last.as_str());
    current.insert(last.clone(), coerce(value, current_value, append));

    Ok(())
}

fn coerce(value: Value, current: Option<&Value>, append: bool) -> Value {
    let Some(Value::Array(items)) = current else {
        return value;
    };

    match value {
        Value::Array(_) => value,
        Value::String(raw) => {
            if let Some(list) = bracket_list(&raw) {
                return Value::Array(list);
            }

            if !append {
                return Value::Array(vec![Value::String(raw)]);
            }

            let mut merged: Vec<Value> = items.clone();
            merged.push(Value::String(raw));
            Value::Array(merged)
        }
        other => Value::Array(vec![other]),
    }
}

fn bracket_list(raw: &str) -> Option<Vec<Value>> {
    let inner: &str = raw.strip_prefix('[')?.strip_suffix(']')?;
    let items: Vec<Value> = inner
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| Value::String(item.trim_matches(['"', '\'']).to_string()))
        .collect();

    Some(items)
}

fn parse_value(raw: &str) -> Value {
    if let Ok(table) = toml::from_str::<Table>(&format!("value = {raw}")) {
        if let Some(value) = table.get("value") {
            return value.clone();
        }
    }

    Value::String(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::{ConfigOverride, apply, parse_value};
    use crate::config::AppConfig;
    use toml::Value;

    fn overrides(raws: &[&str]) -> Vec<ConfigOverride> {
        raws.iter()
            .map(|raw| ConfigOverride::parse(raw).unwrap())
            .collect()
    }

    #[test]
    fn parses_key_and_value() {
        let item: ConfigOverride =
            ConfigOverride::parse("FEATURE_FLAGS.IS_MERGE_PATCH=false").unwrap();

        assert_eq!(item.path, ["FEATURE_FLAGS", "IS_MERGE_PATCH"]);
        assert_eq!(item.value, Value::Boolean(false));
    }

    #[test]
    fn rejects_missing_value() {
        assert!(ConfigOverride::parse("FEATURE_FLAGS.IS_MERGE_PATCH").is_err());
    }

    #[test]
    fn unparsable_value_falls_back_to_string() {
        assert_eq!(parse_value("CN"), Value::String("CN".into()));
        assert_eq!(parse_value("60"), Value::Integer(60));
        assert_eq!(parse_value("[\"a\"]").as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn overrides_feature_flag_by_full_key() {
        let cfg: AppConfig = apply(
            &AppConfig::default(),
            &overrides(&["FEATURE_FLAGS.IS_MERGE_PATCH=false"]),
        )
        .unwrap();

        assert!(!cfg.feature_flags.is_merge_patch);
    }

    #[test]
    fn overrides_feature_flag_by_shorthand() {
        let cfg: AppConfig = apply(
            &AppConfig::default(),
            &overrides(&["IS_DOWNLOAD_RESOURCE=false"]),
        )
        .unwrap();

        assert!(!cfg.feature_flags.is_download_resource);
    }

    #[test]
    fn overrides_nested_server_entry() {
        let cfg: AppConfig = apply(
            &AppConfig::default(),
            &overrides(&["SERVERS.CN.URL=https://example.com"]),
        )
        .unwrap();

        assert_eq!(cfg.servers["CN"].url, "https://example.com");
    }

    #[test]
    fn list_key_accepts_bracket_syntax() {
        let cfg: AppConfig = apply(
            &AppConfig::default(),
            &overrides(&["LAUNCHER.FILES=[a.dll,b.dat]"]),
        )
        .unwrap();

        assert_eq!(cfg.launcher.files, ["a.dll", "b.dat"]);
    }

    #[test]
    fn list_key_accepts_repeated_scalars() {
        let cfg: AppConfig = apply(
            &AppConfig::default(),
            &overrides(&["FILES=a.dll", "FILES=b.dat"]),
        )
        .unwrap();

        assert_eq!(cfg.launcher.files, ["a.dll", "b.dat"]);
    }

    #[test]
    fn list_key_is_replaced_on_first_override() {
        let cfg: AppConfig = apply(&AppConfig::default(), &overrides(&["FILES=a.dll"])).unwrap();

        assert_eq!(cfg.launcher.files, ["a.dll"]);
    }

    #[test]
    fn unknown_key_is_rejected() {
        let base: AppConfig = AppConfig::default();

        assert!(apply(&base, &overrides(&["FEATURE_FLAGS.NOPE=false"])).is_err());
        assert!(apply(&base, &overrides(&["NOPE=false"])).is_err());
    }

    #[test]
    fn empty_overrides_returns_clone() {
        let base: AppConfig = AppConfig::default();
        let cfg: AppConfig = apply(&base, &[]).unwrap();

        assert_eq!(cfg.default_region, base.default_region);
    }
}
