use crate::error::{AppError, Result};
use crate::model::Region;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LauncherSource {
    /// 下载配置里列出的全部区域
    #[default]
    AllRegions,
    /// 只下载命令行指定的区域
    Region(Region),
}

impl LauncherSource {
    pub fn resolve(region: Option<String>) -> Self {
        match normalize_region(region) {
            Some(region) => Self::Region(region),
            None => Self::AllRegions,
        }
    }

    pub fn matches(&self, region: &Region) -> bool {
        match self {
            Self::AllRegions => true,
            Self::Region(target) => target == region,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LauncherFilter {
    /// 资源名筛选正则
    pub patterns: Vec<String>,
    /// 指定下载的文件名
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherSelect {
    /// 清单里的全部文件
    All,
    /// 正则命中的文件
    Regex(Vec<String>),
    /// 列表里指定的文件
    Files(Vec<String>),
}

impl LauncherSelect {
    pub fn resolve(
        all: bool,
        regex: Option<bool>,
        cli: LauncherFilter,
        config: LauncherFilter,
    ) -> Result<Self> {
        if all {
            return Ok(Self::All);
        }

        if !cli.files.is_empty() {
            return Ok(Self::Files(cli.files));
        }

        if regex == Some(false) {
            if config.files.is_empty() {
                return Err(AppError::message(
                    "launcher regex is off but no file selected: add -f/--file <NAME> or set [LAUNCHER].FILES",
                ));
            }
            return Ok(Self::Files(config.files));
        }

        if !cli.patterns.is_empty() {
            return Ok(Self::Regex(cli.patterns));
        }

        if regex == Some(true) {
            if config.patterns.is_empty() {
                return Err(AppError::message(
                    "launcher regex is on but no pattern: add -p/--pattern <REGEX> or set [LAUNCHER].PATTERNS",
                ));
            }
            return Ok(Self::Regex(config.patterns));
        }

        Err(AppError::message(
            "no launcher resource selected: use -a (all files), -p <REGEX> (regex), -f <NAME> (file list), -e ([LAUNCHER].PATTERNS) or -e false ([LAUNCHER].FILES)",
        ))
    }

    pub fn label(&self) -> String {
        match self {
            Self::All => "all".to_string(),
            Self::Regex(patterns) => format!("regex({})", patterns.len()),
            Self::Files(files) => format!("files({})", files.len()),
        }
    }
}

fn normalize_region(region: Option<String>) -> Option<Region> {
    region
        .filter(|value| !value.trim().is_empty())
        .map(|value| Region::new(value.trim().to_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::{LauncherFilter, LauncherSelect, LauncherSource};
    use crate::model::Region;

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
            LauncherSelect::resolve(false, Some(true), cli(&[], &["boot.config"]), config())
                .unwrap();

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
        let err =
            LauncherSelect::resolve(false, Some(true), cli(&[], &[]), LauncherFilter::default())
                .unwrap_err();

        assert!(err.to_string().contains("no pattern"));
    }

    #[test]
    fn regex_off_without_file_is_an_error() {
        let err =
            LauncherSelect::resolve(false, Some(false), cli(&[], &[]), LauncherFilter::default())
                .unwrap_err();

        assert!(err.to_string().contains("no file selected"));
    }
}
