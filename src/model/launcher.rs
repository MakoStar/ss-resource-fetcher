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
