use std::collections::HashSet;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow, bail};
use toml::{Table, Value};

use crate::config::AppConfig;

static OVERRIDES: OnceLock<Vec<ConfigOverride>> = OnceLock::new();

pub(crate) const SHORTHAND_SECTIONS: [&str; 7] = [
    "GLOBAL",
    "NETWORK",
    "FILE_EXTENSION",
    "PATCH_UPDATE_TAG",
    "TASK.CLIENT",
    "TASK.LAUNCHER",
    "TASK.UNCENSOR",
];

#[derive(Debug, Clone)]
pub struct ConfigOverride {
    /// 配置项路径
    pub path: Vec<String>,
    /// 命令行原始输入
    pub raw: String,
    /// 解析后的值 None 表示删除该键
    pub value: Option<Value>,
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
            value: Some(parse_value(value.trim())),
        })
    }

    pub fn parse_del(raw: &str) -> Result<Self> {
        let key: &str = raw.trim();
        if key.is_empty() {
            bail!("invalid delete '{raw}', key is empty");
        }

        if key.contains('=') {
            bail!("invalid delete '{raw}', --del takes a key only, use --set for KEY=VALUE");
        }

        Ok(Self {
            path: key.split('.').map(str::to_string).collect(),
            raw: raw.to_string(),
            value: None,
        })
    }
}

fn apply(cfg: &AppConfig, items: &[ConfigOverride]) -> Result<AppConfig> {
    if items.is_empty() {
        return Ok(cfg.clone());
    }

    let mut root = Table::try_from(cfg).map_err(|err| anyhow!("{err}"))?;
    let mut touched = HashSet::new();

    for item in items {
        let path = resolve_path(&root, item)?;
        let append = touched.contains(&path);

        match &item.value {
            Some(value) => set_value(&mut root, &path, value.clone(), append)?,
            None => remove_value(&mut root, &path)?,
        }
        touched.insert(path);
        log::debug!("config override: {}", item.raw);
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

    if let Some(path) = find_key_path(root, key) {
        return Ok(path);
    }

    bail!("unknown config key '{key}', use SECTION.KEY like TASK.CLIENT.FLAGS.OP.MERGE_PATCH")
}

fn find_key_path(table: &Table, key: &str) -> Option<Vec<String>> {
    if table.contains_key(key) {
        return Some(vec![key.to_string()]);
    }

    for (k, value) in table {
        if let Value::Table(child) = value {
            if let Some(mut sub) = find_key_path(child, key) {
                let mut path = vec![k.to_string()];
                path.append(&mut sub);
                return Some(path);
            }
        }
    }

    None
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

    let current_value = current.get(last.as_str());
    current.insert(last.clone(), coerce(value, current_value, append));

    Ok(())
}

fn remove_value(root: &mut Table, path: &[String]) -> Result<()> {
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

    if current.remove(last.as_str()).is_none() {
        bail!(
            "unknown config key '{}': '{}' not found",
            path.join("."),
            last
        );
    }

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

            let mut merged = items.clone();
            merged.push(Value::String(raw));
            Value::Array(merged)
        }
        other => Value::Array(vec![other]),
    }
}

fn bracket_list(raw: &str) -> Option<Vec<Value>> {
    let inner: &str = raw.strip_prefix('[')?.strip_suffix(']')?;
    let items = inner
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

    fn dels(keys: &[&str]) -> Vec<ConfigOverride> {
        keys.iter()
            .map(|key| ConfigOverride::parse_del(key).unwrap())
            .collect()
    }

    #[test]
    fn parses_key_and_value() {
        let item = ConfigOverride::parse("TASK.CLIENT.FLAGS.OP.MERGE_PATCH=false").unwrap();

        assert_eq!(item.path, ["TASK", "CLIENT", "FLAGS", "OP", "MERGE_PATCH"]);
        assert_eq!(item.value, Some(Value::Boolean(false)));
    }

    #[test]
    fn rejects_missing_value() {
        assert!(ConfigOverride::parse("TASK.CLIENT.FLAGS.OP.MERGE_PATCH").is_err());
    }

    #[test]
    fn del_rejects_value_syntax() {
        assert!(ConfigOverride::parse_del("TASK.CLIENT.SERVERS.CN=https://x").is_err());
        assert!(ConfigOverride::parse_del("").is_err());
    }

    #[test]
    fn del_removes_map_entry() {
        let cfg = apply(&AppConfig::default(), &dels(&["TASK.CLIENT.SERVERS.CN"])).unwrap();

        assert!(!cfg.task.client.servers.contains_key("CN"));
        assert!(cfg.task.client.servers.contains_key("EN"));
    }

    #[test]
    fn del_removes_header_entry() {
        let cfg = apply(
            &AppConfig::default(),
            &dels(&["NETWORK.HEADERS.User-Agent"]),
        )
        .unwrap();

        assert!(!cfg.network.headers.contains_key("User-Agent"));
        assert!(cfg.network.headers.contains_key("Accept-Encoding"));
    }

    #[test]
    fn del_unknown_key_is_rejected() {
        assert!(apply(&AppConfig::default(), &dels(&["NOPE"])).is_err());
    }

    #[test]
    fn del_and_set_combine() {
        let mut items = dels(&["TASK.CLIENT.SERVERS.CN"]);
        items.extend(overrides(&["TASK.CLIENT.FLAGS.OP.MERGE_PATCH=false"]));

        let cfg = apply(&AppConfig::default(), &items).unwrap();

        assert!(!cfg.task.client.servers.contains_key("CN"));
        assert!(!cfg.task.client.flags.op.merge_patch);
    }

    #[test]
    fn unparsable_value_falls_back_to_string() {
        assert_eq!(parse_value("CN"), Value::String("CN".into()));
        assert_eq!(parse_value("60"), Value::Integer(60));
        assert_eq!(parse_value("[\"a\"]").as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn overrides_feature_flag_by_full_key() {
        let cfg = apply(
            &AppConfig::default(),
            &overrides(&["TASK.CLIENT.FLAGS.OP.MERGE_PATCH=false"]),
        )
        .unwrap();

        assert!(!cfg.task.client.flags.op.merge_patch);
    }

    #[test]
    fn overrides_feature_flag_by_shorthand() {
        let cfg = apply(
            &AppConfig::default(),
            &overrides(&["DOWNLOAD_RESOURCE=false"]),
        )
        .unwrap();

        assert!(!cfg.task.client.flags.op.download_resource);
    }

    #[test]
    fn overrides_nested_server_entry() {
        let cfg = apply(
            &AppConfig::default(),
            &overrides(&["TASK.CLIENT.SERVERS.CN.URL=https://example.com"]),
        )
        .unwrap();

        assert_eq!(cfg.task.client.servers["CN"].url, "https://example.com");
    }

    #[test]
    fn list_key_accepts_bracket_syntax() {
        let cfg = apply(
            &AppConfig::default(),
            &overrides(&["TASK.LAUNCHER.REGISTRY.FETCH_FILES=[a.dll,b.dat]"]),
        )
        .unwrap();

        assert_eq!(cfg.task.launcher.registry.fetch_files, ["a.dll", "b.dat"]);
    }

    #[test]
    fn list_key_accepts_repeated_scalars() {
        let cfg = apply(
            &AppConfig::default(),
            &overrides(&["FETCH_FILES=a.dll", "FETCH_FILES=b.dat"]),
        )
        .unwrap();

        assert_eq!(cfg.task.client.registry.fetch_files, ["a.dll", "b.dat"]);
    }

    #[test]
    fn list_key_is_replaced_on_first_override() {
        let cfg = apply(&AppConfig::default(), &overrides(&["FETCH_FILES=a.dll"])).unwrap();

        assert_eq!(cfg.task.client.registry.fetch_files, ["a.dll"]);
    }

    #[test]
    fn unknown_key_is_rejected() {
        let base = AppConfig::default();

        assert!(apply(&base, &overrides(&["TASK.CLIENT.FLAGS.OP.NOPE=false"])).is_err());
        assert!(apply(&base, &overrides(&["NOPE=false"])).is_err());
    }

    #[test]
    fn empty_overrides_returns_clone() {
        let base = AppConfig::default();
        let cfg = apply(&base, &[]).unwrap();

        assert_eq!(cfg.global.default_region, base.global.default_region);
    }
}
