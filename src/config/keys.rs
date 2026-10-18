use anyhow::Result;
use owo_colors::{OwoColorize, Stream};
use toml::{Table, Value};

use super::overrides::SHORTHAND_SECTIONS;
use super::style::{
    HEADER_PREFIX, HEADER_STYLE, KEY_STYLE, LEGEND_STYLE, NOTE_STYLE, TITLE_STYLE, TYPE_STYLE,
    VALUE_STYLE,
};
use crate::config::AppConfig;

const HEADER_WIDTH: usize = 100;

const SPLIT_SECTION: &str = "TASK";

#[derive(Clone)]
struct Row {
    /// 配置键全路径
    key: String,
    /// 值类型
    kind: String,
    /// 内置默认值
    default: String,
}

struct Section {
    /// 分类标题 即该分类的段路径
    title: String,
    /// 分类下的全部键 扁平展示 不再分子段
    rows: Vec<Row>,
}

enum Line {
    /// 首行统计
    Title(String),
    /// 分类分割头
    Header(String),
    /// 列头
    Legend(String),
    /// 键行
    Row(Row),
    /// 说明
    Note(String),
    /// 空行
    Blank,
}

struct Listing {
    /// 键列宽度
    width: usize,
    lines: Vec<Line>,
}

impl Row {
    fn parts(&self, width: usize) -> (String, String, String) {
        (
            format!("{:<width$}", self.key),
            format!("{:<13}", self.kind),
            self.default.clone(),
        )
    }

    fn styled(&self, width: usize) -> String {
        let (key, kind, default) = self.parts(width);

        format!(
            "{}  {}  {}",
            key.if_supports_color(Stream::Stdout, |text| text.style(KEY_STYLE)),
            kind.if_supports_color(Stream::Stdout, |text| text.style(TYPE_STYLE)),
            default.if_supports_color(Stream::Stdout, |text| text.style(VALUE_STYLE))
        )
    }
}

pub fn print_keys() {
    let Listing { width, lines } = build();

    for line in &lines {
        match line {
            Line::Title(text) => println!(
                "{}",
                text.if_supports_color(Stream::Stdout, |text| text.style(TITLE_STYLE))
            ),
            Line::Header(text) => println!(
                "{}",
                text.if_supports_color(Stream::Stdout, |text| text.style(HEADER_STYLE))
            ),
            Line::Legend(text) => println!(
                "{}",
                text.if_supports_color(Stream::Stdout, |text| text.style(LEGEND_STYLE))
            ),
            Line::Row(row) => println!("{}", row.styled(width)),
            Line::Note(text) => println!(
                "{}",
                text.if_supports_color(Stream::Stdout, |text| text.style(NOTE_STYLE))
            ),
            Line::Blank => println!(),
        }
    }
}

fn build() -> Listing {
    let Ok(root_table) = table(&AppConfig::default()) else {
        return Listing {
            width: 0,
            lines: vec![Line::Note("failed to serialize default config".to_string())],
        };
    };

    let mut sections: Vec<Section> = Vec::new();
    collect(&root_table, None, &mut sections);

    let count = sections
        .iter()
        .map(|section| section.rows.len())
        .sum::<usize>();
    let width = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| row.key.chars().count())
        .max()
        .unwrap_or(0);

    let mut lines = vec![
        Line::Title(format!(
            "{count} keys | -s KEY=VALUE | built-in defaults | config order"
        )),
        Line::Blank,
        Line::Legend(legend(width)),
    ];

    for section in &sections {
        lines.push(Line::Header(header(&section.title, section.rows.len())));
        lines.extend(section.rows.iter().cloned().map(Line::Row));
        lines.push(Line::Blank);
    }

    lines.push(Line::Note(format!(
        "shorthand: {}",
        SHORTHAND_SECTIONS.join(" ")
    )));

    lines.push(Line::Note(
        "array: [a,b] replaces, repeat to append".to_string(),
    ));

    Listing { width, lines }
}

fn header(title: &str, count: usize) -> String {
    let title = format!("{HEADER_PREFIX}{title} ({count}) ");
    let fill = HEADER_WIDTH.saturating_sub(title.chars().count());

    format!("{title}{}", "-".repeat(fill))
}

fn legend(width: usize) -> String {
    format!("{:<width$}  {:<13}  {}", "KEY", "TYPE", "VALUE")
}

fn table(cfg: &AppConfig) -> Result<Table> {
    Table::try_from(cfg).map_err(|err| anyhow::anyhow!("{err}"))
}

fn collect(table: &Table, prefix: Option<&str>, out: &mut Vec<Section>) {
    for (key, value) in table {
        let path = match prefix {
            Some(parent) => format!("{parent}.{key}"),
            None => key.clone(),
        };

        match value {
            Value::Table(next) => collect(next, Some(&path), out),
            other => {
                let row = Row {
                    key: path.clone(),
                    kind: type_name(other).to_string(),
                    default: display(other),
                };
                let title = section_of(&path);

                match out.iter_mut().find(|section| section.title == title) {
                    Some(section) => section.rows.push(row),
                    None => out.push(Section {
                        title,
                        rows: vec![row],
                    }),
                }
            }
        }
    }
}

fn section_of(path: &str) -> String {
    let mut parts = path.split('.');
    let head = parts.next().unwrap_or(path).to_string();

    if head == SPLIT_SECTION {
        if let Some(second) = parts.next() {
            return format!("{head}.{second}");
        }
    }

    head
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Boolean(_) => "bool",
        Value::Integer(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(items) => match items.first() {
            Some(Value::String(_)) => "array<string>",
            Some(Value::Integer(_)) => "array<int>",
            Some(Value::Boolean(_)) => "array<bool>",
            _ => "array",
        },
        _ => "value",
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::String(text) if text.is_empty() => "\"\"".to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => {
            let inner: Vec<_> = items.iter().map(display).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Table(_) => "-".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Line, Listing, Row, build};

    impl Line {
        fn plain(&self, width: usize) -> String {
            match self {
                Self::Title(text) | Self::Header(text) | Self::Legend(text) | Self::Note(text) => {
                    text.clone()
                }
                Self::Row(row) => row.plain(width),
                Self::Blank => String::new(),
            }
        }
    }

    impl Row {
        fn plain(&self, width: usize) -> String {
            let (key, kind, default) = self.parts(width);

            format!("{key}  {kind}  {default}")
        }
    }

    fn lines() -> Vec<String> {
        let Listing { width, lines } = build();

        lines.iter().map(|line| line.plain(width)).collect()
    }

    #[test]
    fn lists_keys_with_type_and_default() {
        let out = lines();
        let flag: &String = out
            .iter()
            .find(|line| line.starts_with("TASK.CLIENT.FLAGS.OP.MERGE_PATCH"))
            .expect("missing TASK.CLIENT.FLAGS.OP.MERGE_PATCH");

        assert!(flag.contains("bool"));
        assert!(flag.ends_with("true"));
        assert!(
            out.iter()
                .any(|line| line.starts_with("TASK.CLIENT.SERVERS.CN.URL"))
        );
        assert!(out.iter().any(|line| line.contains("shorthand:")));
    }

    #[test]
    fn groups_keys_by_section() {
        let out = lines();

        assert!(out.iter().any(|line| line.starts_with("-- GLOBAL (")));
        assert!(out.iter().any(|line| line.starts_with("-- TASK.CLIENT (")));

        let head = out
            .iter()
            .position(|line| line.starts_with("-- TASK.CLIENT ("))
            .expect("missing TASK.CLIENT section");
        let url = out
            .iter()
            .position(|line| line.starts_with("TASK.CLIENT.SERVERS.CN.URL"))
            .expect("missing TASK.CLIENT.SERVERS.CN.URL");

        assert!(url > head);
    }

    #[test]
    fn prints_every_key_flat_without_indent() {
        let out = lines();
        let rows: Vec<&String> = out
            .iter()
            .filter(|line| line.starts_with("TASK.CLIENT.") || line.starts_with("GLOBAL."))
            .collect();

        assert!(!rows.is_empty());
        assert!(rows.iter().all(|line| !line.starts_with(' ')));
        assert!(rows.iter().all(|line| !line.contains('·')));
    }

    #[test]
    fn every_section_header_has_the_same_width() {
        let out = lines();
        let widths: Vec<usize> = out
            .iter()
            .filter(|line| line.starts_with("-- "))
            .map(|line| line.chars().count())
            .collect();

        assert!(widths.len() >= 7);
        assert!(widths.iter().all(|width| *width == widths[0]));
    }

    #[test]
    fn lines_stay_plain_so_redirect_is_clean() {
        let out = lines();

        assert!(out.iter().all(|line| !line.contains('\u{1b}')));
    }

    #[test]
    fn splits_task_into_client_launcher_uncensor() {
        let out = lines();
        let titles: Vec<&String> = out.iter().filter(|line| line.starts_with("-- ")).collect();

        assert!(
            titles
                .iter()
                .any(|line| line.starts_with("-- TASK.CLIENT ("))
        );
        assert!(
            titles
                .iter()
                .any(|line| line.starts_with("-- TASK.LAUNCHER ("))
        );
        assert!(
            titles
                .iter()
                .any(|line| line.starts_with("-- TASK.UNCENSOR ("))
        );
        assert!(!titles.iter().any(|line| line.starts_with("-- TASK (")));
    }
}
