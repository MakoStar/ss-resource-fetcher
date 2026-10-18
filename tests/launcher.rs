//! `model::launcher` 的集成测试（原 `src/model/launcher.rs` 内单测迁移）

use ss_resource_fetcher::model::{LauncherFilter, LauncherSelect, LauncherSource, Region};

#[test]
fn no_region_argument_means_all_regions() {
    assert_eq!(LauncherSource::default(), LauncherSource::AllRegions);
    assert_eq!(
        LauncherSource::resolve(None),
        LauncherSource::AllRegions,
        "no argument should keep every configured region"
    );
}

#[test]
fn blank_region_argument_means_all_regions() {
    assert_eq!(
        LauncherSource::resolve(Some("  ".into())),
        LauncherSource::AllRegions
    );
}

#[test]
fn region_argument_is_normalized() {
    assert_eq!(
        LauncherSource::resolve(Some(" kr ".into())),
        LauncherSource::Region(Region::new("KR"))
    );
}

#[test]
fn all_regions_matches_everything() {
    assert!(LauncherSource::AllRegions.matches(&Region::new("CN")));
    assert!(LauncherSource::AllRegions.matches(&Region::new("TW")));
}

#[test]
fn single_region_filters_others() {
    let source = LauncherSource::Region(Region::new("KR"));

    assert!(source.matches(&Region::new("KR")));
    assert!(!source.matches(&Region::new("JP")));
}

fn config() -> LauncherFilter {
    LauncherFilter {
        patterns: vec![r"^data\.unity3d$".into()],
        files: vec!["app.info".into()],
    }
}

fn cli(patterns: &[&str], files: &[&str]) -> LauncherFilter {
    LauncherFilter {
        patterns: patterns.iter().map(|s| s.to_string()).collect(),
        files: files.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn all_wins_over_everything() {
    let select = LauncherSelect::resolve(true, Some(false), cli(&[], &[]), config()).unwrap();

    assert_eq!(select, LauncherSelect::All);
    assert_eq!(select.label(), "all");
}

#[test]
fn cli_file_list_beats_regex() {
    let select =
        LauncherSelect::resolve(false, Some(true), cli(&[], &["boot.config"]), config()).unwrap();

    assert_eq!(select, LauncherSelect::Files(vec!["boot.config".into()]));
    assert_eq!(select.label(), "files(1)");
}

#[test]
fn cli_patterns_are_used_as_is() {
    let select =
        LauncherSelect::resolve(false, None, cli(&["^app\\.info$"], &[]), config()).unwrap();

    assert_eq!(select, LauncherSelect::Regex(vec!["^app\\.info$".into()]));
    assert_eq!(select.label(), "regex(1)");
}

#[test]
fn regex_flag_reads_config_patterns() {
    let select = LauncherSelect::resolve(false, Some(true), cli(&[], &[]), config()).unwrap();

    assert_eq!(
        select,
        LauncherSelect::Regex(vec![r"^data\.unity3d$".into()])
    );
}

#[test]
fn regex_off_reads_config_files() {
    let select = LauncherSelect::resolve(false, Some(false), cli(&[], &[]), config()).unwrap();

    assert_eq!(select, LauncherSelect::Files(vec!["app.info".into()]));
}

#[test]
fn nothing_selected_is_an_error() {
    let err = LauncherSelect::resolve(false, None, cli(&[], &[]), config()).unwrap_err();

    assert!(err.to_string().contains("no launcher resource selected"));
}

#[test]
fn regex_on_without_pattern_is_an_error() {
    let err = LauncherSelect::resolve(false, Some(true), cli(&[], &[]), LauncherFilter::default())
        .unwrap_err();

    assert!(err.to_string().contains("no pattern"));
}

#[test]
fn regex_off_without_file_is_an_error() {
    let err = LauncherSelect::resolve(false, Some(false), cli(&[], &[]), LauncherFilter::default())
        .unwrap_err();

    assert!(err.to_string().contains("no file selected"));
}
