//! `config::global` 的集成测试（原 `src/config/global.rs` 内单测迁移）

use ss_resource_fetcher::config::{GlobalConfig, resolve_platform_value};

#[test]
fn resolve_route_substitutes_platform() {
    let cfg = GlobalConfig::default();
    assert_eq!(cfg.resolve_route("/meta/{platform}.html"), "/meta/win.html");

    let and = GlobalConfig {
        default_platform: "and".into(),
        ..Default::default()
    };
    assert_eq!(and.resolve_route("/meta/{platform}.html"), "/meta/and.html");
}

#[test]
fn platform_value_follows_default_platform() {
    // 文件里存的是真实值 改平台时按模板反查换掉
    assert_eq!(
        resolve_platform_value("ss_win.mani", "ss_{platform}.mani", "and"),
        "ss_and.mani"
    );
    // 老文件里仍是占位符写法
    assert_eq!(
        resolve_platform_value("ss_{platform}.mani", "ss_{platform}.mani", "ios"),
        "ss_ios.mani"
    );
    // 都不匹配说明是用户自己写死的 原样保留
    assert_eq!(
        resolve_platform_value("my_own.mani", "ss_{platform}.mani", "and"),
        "my_own.mani"
    );
}

#[test]
fn default_platform_is_win() {
    let cfg = GlobalConfig::default();
    assert_eq!(cfg.default_platform, "win");
}
