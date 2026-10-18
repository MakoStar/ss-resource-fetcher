//! `model::progress` 的集成测试（原 `src/model/progress.rs` 内单测迁移）

use ss_resource_fetcher::model::{DownloadProgress, SEPARATOR};

#[test]
fn separator_is_sixty_four_dashes() {
    assert_eq!(SEPARATOR.len(), 64);
    assert!(SEPARATOR.bytes().all(|byte| byte == b'-'));
}

#[test]
fn formats_count_prefix() {
    let progress = DownloadProgress::new(3, 128);
    assert_eq!(progress.prefix(), "[3/128]");
    assert_eq!(progress.to_string(), "[3/128]");
}
