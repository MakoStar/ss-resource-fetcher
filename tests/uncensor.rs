//! `model::uncensor` 的集成测试（原 `src/model/uncensor.rs` 内单测迁移）

use std::path::PathBuf;

use ss_resource_fetcher::model::{Region, UncensorSource};

#[test]
fn default_source_is_config_region() {
    assert_eq!(UncensorSource::default(), UncensorSource::ConfigRegion);
    assert_eq!(
        UncensorSource::resolve(None, false, false, None).unwrap(),
        UncensorSource::ConfigRegion
    );
}

#[test]
fn region_arg_overrides_config() {
    assert_eq!(
        UncensorSource::resolve(Some("tw".into()), false, false, None).unwrap(),
        UncensorSource::Region(Region::new("TW"))
    );
}

#[test]
fn default_url_wins_over_region() {
    assert_eq!(
        UncensorSource::resolve(Some("TW".into()), true, false, None).unwrap(),
        UncensorSource::DefaultUrl
    );
}

#[test]
fn blank_region_falls_back_to_config() {
    assert_eq!(
        UncensorSource::resolve(Some("  ".into()), false, false, None).unwrap(),
        UncensorSource::ConfigRegion
    );
}

#[test]
fn custom_files_requires_path() {
    assert!(UncensorSource::resolve(None, false, true, None).is_err());
}

#[test]
fn custom_files_with_region_override() {
    let source = UncensorSource::resolve(
        Some("jp".into()),
        false,
        true,
        Some(PathBuf::from("list.json")),
    )
    .unwrap();

    assert_eq!(
        source,
        UncensorSource::CustomFiles {
            path: PathBuf::from("list.json"),
            region: Some(Region::new("JP")),
        }
    );
}

#[test]
fn custom_files_without_region_override() {
    let source =
        UncensorSource::resolve(None, false, true, Some(PathBuf::from("list.json"))).unwrap();

    assert_eq!(
        source,
        UncensorSource::CustomFiles {
            path: PathBuf::from("list.json"),
            region: None,
        }
    );
}
