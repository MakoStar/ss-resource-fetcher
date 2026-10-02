use std::path::PathBuf;

use clap::Parser;
use unicode_width::UnicodeWidthStr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// 生成清单记录
    #[arg(short = 'g', long, default_value_t = false)]
    pub generate_manifest_record: bool,

    /// 只下载 Uncensor 资源包
    #[arg(short = 'u', long = "download-uncensor-pack", default_value_t = false)]
    pub download_uncensor_pack: bool,

    /// Uncensor 资源包下载所用区域
    #[arg(
        short = 'r',
        long = "uncensor-region",
        value_name = "REGION",
        conflicts_with = "uncensor_default_url"
    )]
    pub uncensor_region: Option<String>,

    /// 直接使用 Uncensor 默认地址下载资源
    #[arg(short = 'U', long = "uncensor-default-url", default_value_t = false)]
    pub uncensor_default_url: bool,

    /// 使用自定义反和谐文件列表
    #[arg(
        short = 'c',
        long = "uncensor-custom-files",
        requires = "uncensor_custom_files_path",
        conflicts_with = "uncensor_default_url",
        default_value_t = false
    )]
    pub uncensor_custom_files: bool,

    /// 自定义反和谐文件列表 JSON 路径
    #[arg(
        long = "uncensor-custom-files-path",
        value_name = "PATH",
        requires = "uncensor_custom_files"
    )]
    pub uncensor_custom_files_path: Option<PathBuf>,
}

#[macro_export]
macro_rules! print_banner {
    () => {
        $crate::print_banner(env!("CARGO_BIN_NAME"))
    };
    ($t:expr) => {
        $crate::print_banner($t)
    };
}

pub fn print_banner(t: impl AsRef<str>) {
    let title = format!(
        "             StellaSora   v{}    {}         ",
        env!("CARGO_PKG_VERSION"),
        t.as_ref(),
    );
    print_boxed(&[
        title.as_str(),
        "               (╯°□°)╯     ₍^..^₎      ╰(°□°╰)             ",
    ]);
}

fn print_boxed(lines: &[&str]) {
    let max_width = lines.iter().map(|line| line.width()).max().unwrap_or(0);
    let inner_width = max_width + 2;

    logger::tips!("{}", format!("╔{}╗", "═".repeat(inner_width)));
    for line in lines {
        let padding = inner_width - line.width() - 2;
        logger::tips!("║ {}{} ║", line, " ".repeat(padding));
    }
    logger::tips!("{}", format!("╚{}╝", "═".repeat(inner_width)));
}
