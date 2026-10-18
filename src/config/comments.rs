use std::collections::HashSet;

use clap::ValueEnum;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentLang {
    Cn,
    En,
}

const HEADER: &[(&str, &str)] = &[
    ("应用程序配置文件", "Application config file"),
    ("修改后重启应用即可生效", "Restart to apply changes"),
    (
        "-s/--set 段.键=值 可临时覆盖，无需改此文件",
        "-s/--set SECTION.KEY=VALUE overrides at runtime",
    ),
];

const SECTIONS: &[(&str, &str, &str)] = &[
    ("[GLOBAL]", "全局配置", "Global settings"),
    ("[NETWORK.HEADERS]", "请求头", "Request headers"),
    ("[NETWORK]", "网络客户端", "HTTP client"),
    ("[FILE_EXTENSION]", "文件扩展名", "File extensions"),
    (
        "[PATCH_UPDATE_TAG]",
        "补丁标签: u 增量 / n 整文件 / m 清单",
        "Patch tags: u=delta, n=full file, m=manifest",
    ),
    ("[TASK.CLIENT]", "热更客户端任务", "Client task"),
    (
        "[TASK.CLIENT.ROUTE]",
        "客户端路由（平台相关）",
        "Client routes (platform-bound)",
    ),
    (
        "[TASK.CLIENT.SERVERS.",
        "各区域服务器: URL 地址 / KEY 密钥",
        "Per-region servers: URL / decrypt KEY",
    ),
    (
        "[TASK.CLIENT.FILENAME]",
        "客户端文件名（平台相关）",
        "Client file names (platform-bound)",
    ),
    (
        "[TASK.CLIENT.FILEPATH]",
        "客户端输出目录",
        "Client output paths",
    ),
    (
        "[TASK.CLIENT.REGISTRY]",
        "下载与解包范围",
        "Download and unpack scope",
    ),
    (
        "[TASK.CLIENT.EXTRACTKEY]",
        "版本提取键",
        "Version extract keys",
    ),
    (
        "[TASK.CLIENT.FLAGS.SAVE]",
        "客户端保存开关",
        "Client save flags",
    ),
    (
        "[TASK.CLIENT.FLAGS.OP]",
        "客户端运行开关",
        "Client run flags",
    ),
    ("[TASK.LAUNCHER]", "Launcher 任务", "Launcher task"),
    ("[TASK.LAUNCHER.ROUTE]", "Launcher 路由", "Launcher routes"),
    (
        "[TASK.LAUNCHER.SERVERS.",
        "各区域端点:TAG / SALT / API / PKG",
        "Per-region endpoints: TAG / SALT / API / PKG",
    ),
    (
        "[TASK.LAUNCHER.FILENAME]",
        "Launcher 文件名",
        "Launcher file names",
    ),
    (
        "[TASK.LAUNCHER.FILEPATH]",
        "Launcher 输出目录",
        "Launcher output paths",
    ),
    (
        "[TASK.LAUNCHER.REGISTRY]",
        "下载范围（须显式指定）",
        "Download scope (required)",
    ),
    (
        "[TASK.LAUNCHER.EXTRACTKEY]",
        "版本提取键",
        "Version extract keys",
    ),
    (
        "[TASK.LAUNCHER.FLAGS.SAVE]",
        "Launcher 保存开关",
        "Launcher save flags",
    ),
    (
        "[TASK.LAUNCHER.FLAGS.OP]",
        "Launcher 运行开关",
        "Launcher run flags",
    ),
    ("[TASK.UNCENSOR]", "反和谐任务", "Uncensor task"),
    (
        "[TASK.UNCENSOR.ROUTE]",
        "反和谐路由（平台相关）",
        "Uncensor route (platform-bound)",
    ),
    (
        "[TASK.UNCENSOR.SERVERS.",
        "反和谐清单服务器",
        "Uncensor manifest server",
    ),
    (
        "[TASK.UNCENSOR.FILENAME]",
        "反和谐文件名（平台相关）",
        "Uncensor file names (platform-bound)",
    ),
    (
        "[TASK.UNCENSOR.FILEPATH]",
        "反和谐输出目录",
        "Uncensor output paths",
    ),
];

const BAR_WIDTH: usize = 64;

pub fn apply(content: &str, lang: CommentLang) -> String {
    let mut out = Vec::new();

    let header: Vec<&str> = HEADER.iter().map(|(zh, en)| pick(lang, zh, en)).collect();
    out.push(wrap(&header));
    out.push(String::new());

    let mut inserted = HashSet::new();

    for line in content.lines() {
        if let Some(key) = section_key(line)
            && let Some((zh, en)) = section_comment(&key)
            && inserted.insert(key.clone())
        {
            let text = pick(lang, zh, en);
            let tagged = match task_prefix(&key) {
                Some(prefix) => format!("{prefix}  {text}"),
                None => text.to_string(),
            };

            out.push(wrap(&[&tagged]));
        }

        out.push(line.to_string());
    }

    let mut result = out.join("\n");
    result.push('\n');

    result
}

fn wrap(lines: &[&str]) -> String {
    let bar = "=".repeat(BAR_WIDTH);
    let mut block = format!("# {bar}\n");

    for line in lines {
        block.push_str(&format!("# {line}\n"));
    }

    block.push_str(&format!("# {bar}"));

    block
}

fn pick<'a>(lang: CommentLang, zh: &'a str, en: &'a str) -> &'a str {
    match lang {
        CommentLang::Cn => zh,
        CommentLang::En => en,
    }
}

fn section_key(line: &str) -> Option<String> {
    if !line.starts_with('[') {
        return None;
    }

    let end = line.find(']')?;
    let raw = &line[..=end];

    for prefix in [
        "[TASK.CLIENT.SERVERS.",
        "[TASK.LAUNCHER.SERVERS.",
        "[TASK.UNCENSOR.SERVERS.",
    ] {
        if raw.starts_with(prefix) {
            return Some(prefix.to_string());
        }
    }

    Some(raw.to_string())
}

fn task_prefix(key: &str) -> Option<&'static str> {
    if key.starts_with("[TASK.CLIENT") {
        Some("TASK.CLIENT:")
    } else if key.starts_with("[TASK.LAUNCHER") {
        Some("TASK.LAUNCHER:")
    } else if key.starts_with("[TASK.UNCENSOR") {
        Some("TASK.UNCENSOR:")
    } else {
        None
    }
}

fn section_comment(key: &str) -> Option<(&'static str, &'static str)> {
    if let Some((_, zh, en)) = SECTIONS.iter().find(|(k, _, _)| *k == key) {
        return Some((zh, en));
    }

    SECTIONS
        .iter()
        .find(|(k, _, _)| key.starts_with(*k))
        .map(|(_, zh, en)| (*zh, *en))
}

#[cfg(test)]
mod tests {
    use super::{CommentLang, apply};

    #[test]
    fn comments_only_section_headers() {
        let input = "[GLOBAL]\nDEFAULT_REGION = \"CN\"\nDEFAULT_PLATFORM = \"win\"\n";
        let out = apply(input, CommentLang::En);

        let commented: Vec<&str> = out
            .lines()
            .filter(|line| line.starts_with('#') && !line.starts_with("# ="))
            .collect();
        assert_eq!(
            commented,
            [
                "# Application config file",
                "# Restart to apply changes",
                "# -s/--set SECTION.KEY=VALUE overrides at runtime",
                "# Global settings"
            ]
        );
        assert_eq!(out.matches("# ==").count(), 4);
        assert!(!out.contains("# Default region"));
        assert!(!out.contains("# Default platform"));
    }

    #[test]
    fn switches_language_by_lang() {
        let input = "[GLOBAL]\nDEFAULT_REGION = \"CN\"\n";

        let cn = apply(input, CommentLang::Cn);
        assert!(cn.contains("# 全局配置"));
        assert!(!cn.contains("# Global settings"));

        let en = apply(input, CommentLang::En);
        assert!(en.contains("# Global settings"));
        assert!(!en.contains("# 全局配置"));
    }

    #[test]
    fn tags_task_sections_with_domain_prefix() {
        let input = "[TASK.CLIENT.FLAGS.OP]\nMERGE_PATCH = true\n";
        let out = apply(input, CommentLang::En);

        assert!(out.contains("# TASK.CLIENT:  Client run flags"));

        let global = apply("[NETWORK]\nVERIFY_TLS = false\n", CommentLang::En);
        assert!(global.contains("# HTTP client"));
        assert!(!global.contains("TASK"));
    }

    #[test]
    fn repeated_server_tables_commented_once() {
        let input =
            "[TASK.CLIENT.SERVERS.CN]\nurl = \"a\"\n\n[TASK.CLIENT.SERVERS.EN]\nurl = \"b\"\n";
        let out = apply(input, CommentLang::En);

        assert_eq!(out.matches("Per-region servers").count(), 1);
        assert!(out.contains("[TASK.CLIENT.SERVERS.CN]"));
        assert!(out.contains("[TASK.CLIENT.SERVERS.EN]"));
    }

    #[test]
    fn comment_precedes_its_section() {
        let input = "[NETWORK]\nVERIFY_TLS = false\n";
        let out = apply(input, CommentLang::Cn);

        let comment = out.find("# 网络客户端").unwrap();
        let section = out.find("[NETWORK]").unwrap();
        assert!(comment < section);
    }

    #[test]
    fn preserves_original_content() {
        let input = "[FILE_EXTENSION]\nMANI = \".mani\"\n";
        let out = apply(input, CommentLang::En);

        for line in input.lines() {
            assert!(out.contains(line), "lost original line: {line}");
        }
    }
}
