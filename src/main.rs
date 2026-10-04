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

use crate::cli::{Args, Task};
use crate::config::{
    AppConfig, ConfigOverride, set_config_comments, set_config_init, set_config_overrides,
};
use crate::error::Result;
use crate::model::{LauncherFilter, LauncherSource, UncensorSource};
use crate::pipeline::{LauncherTask, ResourcePipeline, TaskOptions};

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_logger!();

    let args = Args::parse();
    print_banner!();

    set_config_comments(args.comments);
    set_config_init(args.init);

    let mut overrides: Vec<ConfigOverride> = Vec::with_capacity(args.set.len());
    for raw in &args.set {
        overrides.push(ConfigOverride::parse(raw)?);
    }
    set_config_overrides(overrides);

    let task: TaskOptions = match args.task {
        Some(Task::Uncensor {
            region,
            default,
            custom,
            path,
        }) => TaskOptions::Uncensor {
            source: UncensorSource::resolve(region, default, custom, path)?,
        },
        Some(Task::Launcher {
            region,
            all,
            pattern,
            file,
            regex,
            keep,
        }) => TaskOptions::Launcher(LauncherTask {
            source: LauncherSource::resolve(region),
            all,
            regex,
            filter: LauncherFilter {
                patterns: pattern,
                files: file,
            },
            keep,
        }),
        Some(Task::Run { record }) if record => TaskOptions::Record,
        _ => TaskOptions::Pipeline,
    };

    ResourcePipeline::new(AppConfig::get()?).run(&task).await
}
