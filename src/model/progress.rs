pub const SEPARATOR: &str = concat!(
    "--------", "--------", "--------", "--------", "--------", "--------", "--------", "--------"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    /// 当前序号
    current: usize,
    /// 总数
    total: usize,
}

impl DownloadProgress {
    pub fn new(current: usize, total: usize) -> Self {
        Self { current, total }
    }

    pub fn prefix(&self) -> String {
        format!("[{}/{}]", self.current, self.total)
    }
}

impl std::fmt::Display for DownloadProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}/{}]", self.current, self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::{DownloadProgress, SEPARATOR};

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
}
