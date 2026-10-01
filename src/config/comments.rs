use std::collections::HashSet;

const HEADER_TITLE: &str = "应用程序配置文件";
const HEADER_NOTES: &[&str] = &["修改后重启应用即可生效"];

const SECTION_COMMENTS: &[(&str, &str, &[&str])] = &[
    ("[SERVER_ROUTE]", "服务器路由配置", &[]),
    (
        "[FILE_PATH]",
        "文件路径配置",
        &["注意：所有路径都是相对于程序运行目录"],
    ),
    ("[FILE_NAME]", "文件名配置", &[]),
    (
        "[EXTRACTOR]",
        "提取器配置",
        &["当前应用于 .mani 文件 version 提取 key (DO NOT EDIT)"],
    ),
    (
        "[REQUEST]",
        "HTTP 请求配置",
        &[
            "如果网络不好或者文件太大, 请求读取 body 会超时",
            "可适当增加 TIMEOUT_SECS 来避免(特别是 lua.arcx )",
        ],
    ),
    ("[RESOURCE_REGISTRY]", "资源注册表配置", &[]),
    (
        "[UNCENSOR]",
        "Uncensor 资源包配置",
        &[
            "命令行加 -u / --download-uncensor-pack 才会执行（独占任务，不跑常规流水线）",
            "URL: 第三方 Uncensor 清单服务器",
            "SOURCE_REGION: 实际下载资源所用的官方区域",
        ],
    ),
    (
        "[SERVERS.",
        "服务器配置",
        &[
            "区域名 = { URL = \"服务器地址\", KEY = \"密钥\" }",
            "KEY: 来自于提取的 ClientConfig.json 的 serverMetaKey",
        ],
    ),
    (
        "[FEATURE_FLAGS]",
        "功能开关",
        &[
            "注意: IS_SAVE_PATCH_MANIFEST 必须是 true",
            "因为合并补丁需要读取补丁清单和校验清单中的 hash",
        ],
    ),
];

const KEY_COMMENTS: &[(&str, &str)] = &[(
    "DEFAULT_REGION = ",
    "# 默认区域: 启用 IS_USE_DEFAULT_REGION 时生效",
)];

#[allow(dead_code)]
pub fn apply(content: &str) -> String {
    let mut out: Vec<String> = Vec::new();

    out.push(comment_block(HEADER_TITLE, HEADER_NOTES));
    out.push(String::new());

    let mut inserted: HashSet<&str> = HashSet::new();

    for line in content.lines() {
        if let Some(note) = key_comment(line) {
            out.push(note.to_string());
        }

        if let Some(key) = section_key(line)
            && let Some((_, title, notes)) = SECTION_COMMENTS.iter().find(|(k, _, _)| *k == key)
            && inserted.insert(key)
        {
            out.push(String::new());
            out.push(comment_block(title, notes));
        }

        out.push(line.to_string());
    }

    let mut result: String = out.join("\n");
    result.push('\n');

    result
}

#[allow(dead_code)]
fn section_key(line: &str) -> Option<&str> {
    if !line.starts_with('[') {
        return None;
    }

    let end = line.find(']')?;
    let raw = &line[..=end];

    if raw.starts_with("[SERVERS.") {
        return Some("[SERVERS.");
    }

    if raw.contains('.') {
        return None;
    }

    Some(raw)
}

#[allow(dead_code)]
fn key_comment(line: &str) -> Option<&'static str> {
    KEY_COMMENTS
        .iter()
        .find(|(prefix, _)| line.starts_with(*prefix))
        .map(|(_, note)| *note)
}

#[allow(dead_code)]
fn comment_block(title: &str, notes: &[&str]) -> String {
    let bar: String = "=".repeat(60);
    let mut block = format!("# {bar}\n# {title}\n");

    for note in notes {
        block.push_str(&format!("# {note}\n"));
    }
    block.push_str(&format!("# {bar}"));

    block
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn injects_header_and_section_comments() {
        let out = apply("[FILE_PATH]\nMANIFEST_OUTPUT_DIR = \"./out\"\n");

        assert!(out.starts_with("# ===="));
        assert!(out.contains("# 应用程序配置文件"));
        assert!(out.contains("# 文件路径配置"));
        assert!(out.find("# 文件路径配置").unwrap() < out.find("[FILE_PATH]").unwrap());
    }

    #[test]
    fn comments_repeated_sub_tables_only_once() {
        let input = "[SERVERS.CN]\nURL = \"a\"\n\n[SERVERS.EN]\nURL = \"b\"\n";
        let out = apply(input);

        assert_eq!(out.matches("# 服务器配置").count(), 1);
        assert!(out.contains("[SERVERS.CN]"));
        assert!(out.contains("[SERVERS.EN]"));
    }

    #[test]
    fn leaves_other_sub_tables_alone() {
        let out = apply("[REQUEST]\nTIMEOUT_SECS = 1\n\n[REQUEST.EXTRA_HEADERS]\nA = \"b\"\n");

        assert_eq!(out.matches("# HTTP 请求配置").count(), 1);
        assert_eq!(out.matches("# ====").count(), 4);
    }

    #[test]
    fn annotates_top_level_keys() {
        let out = apply("DEFAULT_REGION = \"CN\"\n");

        assert!(out.contains("# 默认区域"));
        assert!(out.find("# 默认区域").unwrap() < out.find("DEFAULT_REGION").unwrap());
    }

    #[test]
    fn preserves_original_content() {
        let input = "[FILE_NAME]\nVERSION_FILE = \"version.json\"\n";
        let out = apply(input);

        for line in input.lines() {
            assert!(out.contains(line), "lost original line: {line}");
        }
    }
}
