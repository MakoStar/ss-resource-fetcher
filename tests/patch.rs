//! `model::patch` 的集成测试（原 `src/model/patch.rs` 内单测迁移）

use ss_resource_fetcher::config::{AppConfig, ClientRegistryConfig};
use ss_resource_fetcher::model::{PatchRef, apply_order};

fn registry() -> ClientRegistryConfig {
    AppConfig::default().task.client.registry
}

fn refs(patches: &[(&'static str, u64)]) -> Vec<PatchRef<'static>> {
    let registry = registry();

    patches
        .iter()
        .map(|(name, version)| PatchRef::new(&registry, name, *version))
        .collect()
}

fn order(base: Option<u64>, patches: &[(&'static str, u64)]) -> Vec<&'static str> {
    let registry = registry();
    let refs = refs(patches);

    apply_order(base, &refs, |name| registry.is_new_file_patch(name))
}

#[test]
fn new_file_patch_replaces_lower_patches() {
    let kept = order(
        Some(91),
        &[
            ("p_12_n.char_2d_13901.unity3d", 147),
            ("p_1_u.char_2d_13901.unity3d", 146),
        ],
    );

    assert_eq!(kept, vec!["p_12_n.char_2d_13901.unity3d"]);
}

#[test]
fn newer_delta_merges_on_top_of_new_file() {
    let kept = order(
        Some(91),
        &[
            ("p_12_n.char_2d_13901.unity3d", 147),
            ("p_1_u.char_2d_13901.unity3d", 146),
            ("p_13_u.char_2d_13901.unity3d", 148),
        ],
    );

    assert_eq!(
        kept,
        vec![
            "p_12_n.char_2d_13901.unity3d",
            "p_13_u.char_2d_13901.unity3d"
        ]
    );
}

#[test]
fn deltas_without_new_file_keep_every_newer_version() {
    let kept = order(
        Some(91),
        &[
            ("p_1_u.char_2d_13901.unity3d", 120),
            ("p_2_u.char_2d_13901.unity3d", 130),
        ],
    );

    assert_eq!(
        kept,
        vec!["p_1_u.char_2d_13901.unity3d", "p_2_u.char_2d_13901.unity3d"]
    );
}

#[test]
fn latest_new_file_patch_wins() {
    let kept = order(
        Some(91),
        &[
            ("p_5_n.char_2d_13901.unity3d", 100),
            ("p_12_n.char_2d_13901.unity3d", 147),
        ],
    );

    assert_eq!(kept, vec!["p_12_n.char_2d_13901.unity3d"]);
}

#[test]
fn patches_older_than_base_are_dropped() {
    let kept = order(Some(120), &[("p_1_u.char_2d_13901.unity3d", 91)]);

    assert!(kept.is_empty());
    assert_eq!(order(None, &[("p_1_u.char_2d_13901.unity3d", 91)]).len(), 1);
}

/// 同版本的增量补丁排在整文件补丁之后时不能丢
///
/// TW 的 ui_activity__510007.unity3d 只有补丁条目 p_8_n(143) p_10_u(143) p_11_u(146)
/// 丢掉 p_10_u 会让 p_11_u 找不到底座 直接报 bsdiff 失败
#[test]
fn same_version_delta_after_new_file_is_kept() {
    let kept = order(
        None,
        &[
            ("p_8_n.ui_activity__510007.unity3d", 143),
            ("p_10_u.ui_activity__510007.unity3d", 143),
            ("p_11_u.ui_activity__510007.unity3d", 146),
            ("p_12_u.ui_activity__510007.unity3d", 147),
        ],
    );

    assert_eq!(
        kept,
        vec![
            "p_8_n.ui_activity__510007.unity3d",
            "p_10_u.ui_activity__510007.unity3d",
            "p_11_u.ui_activity__510007.unity3d",
            "p_12_u.ui_activity__510007.unity3d",
        ]
    );
}

/// 同版本的多个增量补丁要按序号全部保留 不能按版本号去重
#[test]
fn duplicate_version_deltas_are_all_kept() {
    let kept = order(
        Some(124),
        &[
            ("p_9_u.lua.arcx", 133),
            ("p_8_u.lua.arcx", 133),
            ("p_10_u.lua.arcx", 136),
        ],
    );

    assert_eq!(
        kept,
        vec!["p_8_u.lua.arcx", "p_9_u.lua.arcx", "p_10_u.lua.arcx"]
    );
}

/// 高版本之后的低版本补丁是过时的 不能插回链里
#[test]
fn older_delta_after_newer_is_dropped() {
    let kept = order(
        Some(124),
        &[
            ("p_1_u.lua.arcx", 126),
            ("p_2_u.lua.arcx", 127),
            ("p_5_u.lua.arcx", 107),
            ("p_4_u.lua.arcx", 129),
        ],
    );

    assert_eq!(
        kept,
        vec!["p_1_u.lua.arcx", "p_2_u.lua.arcx", "p_4_u.lua.arcx"]
    );
}
