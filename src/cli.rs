use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// 生成清单记录
    #[arg(short = 'g', long, default_value_t = false)]
    pub generate_manifest_record: bool,

    /// 只下载反和谐资源包
    #[arg(short = 'u', long = "download-uncensor-pack", default_value_t = false)]
    pub download_uncensor_pack: bool,

    /// 反和谐资源包下载所用区域
    #[arg(
        short = 'r',
        long = "uncensor-region",
        value_name = "REGION",
        conflicts_with = "uncensor_default_url"
    )]
    pub uncensor_region: Option<String>,

    /// 直接使用反和谐默认地址下载资源
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
