mod banner;
mod cli;
mod config;
mod error;
mod extractor;
mod generated;
mod manifest;
mod model;
mod network;
mod patch;
mod pipeline;
mod storage;
mod uncensor;

use clap::Parser;

use crate::cli::Args;
use crate::config::AppConfig;
use crate::error::Result;
use crate::model::UncensorSource;
use crate::pipeline::{PipelineOptions, ResourcePipeline};

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_logger!();

    let args = Args::parse();
    print_banner!();

    let options = PipelineOptions {
        generate_manifest_record: args.generate_manifest_record,
        download_uncensor_pack: args.download_uncensor_pack,
        uncensor_source: UncensorSource::resolve(
            args.uncensor_region,
            args.uncensor_default_url,
            args.uncensor_custom_files,
            args.uncensor_custom_files_path,
        )?,
    };

    ResourcePipeline::new(AppConfig::get()).run(&options).await
}
