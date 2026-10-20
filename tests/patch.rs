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

/// 首个补丁与本体同版本时不能丢 否则后面的补丁丢失基准
///
/// 用 <= 判过时会丢掉 p_1_u，p_2_u 拿本体当基准直接报
/// `bsdiff apply failed: ... failed to fill whole buffer`
#[test]
fn first_patch_matching_base_version_is_kept() {
    let kept = order(
        Some(143),
        &[
            ("p_1_u.win.unity3d", 143),
            ("p_2_u.win.unity3d", 145),
            ("p_3_u.win.unity3d", 132),
        ],
    );

    assert_eq!(kept, vec!["p_1_u.win.unity3d", "p_2_u.win.unity3d"]);
}

/// 同版本首个补丁被丢时 后续补丁可能跑完 bsdiff 但结果不对 表现为 hash mismatch
#[test]
fn first_patch_matching_base_version_merges_in_order() {
    let kept = order(
        Some(143),
        &[
            ("p_1_u.ui_soldier.jp.unity3d", 143),
            ("p_2_u.ui_soldier.jp.unity3d", 145),
            ("p_5_u.ui_soldier.jp.unity3d", 134),
            ("p_8_u.ui_soldier.jp.unity3d", 137),
        ],
    );

    assert_eq!(
        kept,
        vec!["p_1_u.ui_soldier.jp.unity3d", "p_2_u.ui_soldier.jp.unity3d"]
    );
}

#[test]
fn first_patch_older_than_base_is_still_dropped() {
    let kept = order(
        Some(143),
        &[("p_1_u.win.unity3d", 132), ("p_2_u.win.unity3d", 145)],
    );

    assert_eq!(kept, vec!["p_2_u.win.unity3d"]);
}
