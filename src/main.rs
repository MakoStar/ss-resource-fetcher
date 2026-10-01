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

use clap::Parser;

use crate::cli::{Args, print_banner};
use crate::config::AppConfig;
use crate::error::Result;
use crate::pipeline::{PipelineOptions, ResourcePipeline};

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_logger!();

    let args = Args::parse();
    print_banner!();

    let options = PipelineOptions {
        generate_manifest_record: args.generate_manifest_record,
        download_uncensor_pack: args.download_uncensor_pack,
    };

    ResourcePipeline::new(AppConfig::get()).run(&options).await
}
