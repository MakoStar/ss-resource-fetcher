use clap::{Command, CommandFactory, FromArgMatches};

use ss_resource_fetcher::cli::{Args, Task};
use ss_resource_fetcher::config::{
    AppConfig, CONFIG_FILE_NAME, ConfigOverride, help_styles, print_keys, resolve_color_choice,
    set_config_comments, set_config_generate, set_config_overrides,
};
use ss_resource_fetcher::error::Result;
use ss_resource_fetcher::model::{LauncherFilter, LauncherSource, UncensorSource};
use ss_resource_fetcher::pipeline::{LauncherTask, ResourcePipeline, TaskOptions};
use ss_resource_fetcher::print_banner;

fn styled_command() -> Command {
    Args::command()
        .styles(help_styles())
        .color(resolve_color_choice())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = {
        let matches = styled_command().get_matches();
        Args::from_arg_matches(&matches).expect("args should build from the matched command")
    };

    if args.list_keys {
        print_keys();
        return Ok(());
    }

    logger::init_logger!(args.log_level);

    let config_only = args.task.is_none() && args.gen_config.unwrap_or(false);
    if args.task.is_none() && !config_only {
        styled_command().print_help()?;
        println!();
        std::process::exit(2);
    }

    print_banner!();

    set_config_comments(args.comments);
    set_config_generate(args.gen_config);

    let mut overrides = Vec::with_capacity(args.set.len() + args.del.len());
    for raw in &args.set {
        overrides.push(ConfigOverride::parse(raw)?);
    }
    for key in &args.del {
        overrides.push(ConfigOverride::parse_del(key)?);
    }
    set_config_overrides(overrides);

    let Some(task) = args.task else {
        let _: &AppConfig = AppConfig::get()?;
        logger::succ!("config ready: {}", CONFIG_FILE_NAME);

        return Ok(());
    };

    let task = match task {
        Task::Client {
            record,
            filter_outdated,
        } => {
            if record {
                TaskOptions::Record
            } else {
                TaskOptions::Pipeline { filter_outdated }
            }
        }
        Task::Launcher {
            region,
            all,
            pattern,
            file,
            regex,
            keep,
        } => TaskOptions::Launcher(LauncherTask {
            source: LauncherSource::resolve(region),
            all,
            regex,
            filter: LauncherFilter {
                patterns: pattern,
                files: file,
            },
            keep,
        }),
        Task::Uncensor {
            region,
            default,
            custom,
            path,
            manifest,
        } => TaskOptions::Uncensor {
            source: UncensorSource::resolve(region, default, custom, path)?,
            save_manifest: manifest.unwrap_or(true),
        },
    };

    ResourcePipeline::new(AppConfig::get()?).run(&task).await
}
