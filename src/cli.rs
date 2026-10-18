use std::path::PathBuf;

use crate::config::CommentLang;
use clap::{Parser, Subcommand};
use log::LevelFilter;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = None,
    bin_name = "ResourceFetcher",
    arg_required_else_help = true
)]
pub struct Args {
    /// Override a config item, repeatable
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", global = true)]
    pub set: Vec<String>,

    /// Write the config file, unset=skip, true=generate (overwrites)
    #[arg(
        short = 'g',
        long = "gen_config",
        value_name = "BOOL",
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true
    )]
    pub gen_config: Option<bool>,

    /// Write comments into the generated config, cn or en
    #[arg(
        short = 'a',
        long = "add_comments",
        value_name = "LANG",
        num_args = 0..=1,
        default_missing_value = "en",
        ignore_case = true,
        value_enum
    )]
    pub comments: Option<CommentLang>,

    /// Log level: off, error, warn, info, debug, trace
    #[arg(
        long = "log_level",
        value_name = "LEVEL",
        global = true,
        default_value = "info"
    )]
    pub log_level: LevelFilter,

    /// Print every -s KEY=VALUE key with type and default
    #[arg(short = 'p', long = "print_keys", default_value_t = false)]
    pub list_keys: bool,

    /// Remove a config item, repeatable
    #[arg(short = 'd', long = "del", value_name = "KEY")]
    pub del: Vec<String>,

    #[command(subcommand)]
    pub task: Option<Task>,
}

#[derive(Subcommand, Debug)]
pub enum Task {
    /// Client resource pipeline
    Client {
        /// Only dump the manifest record
        #[arg(short = 'r', long = "record", default_value_t = false)]
        record: bool,

        /// Filter outdated patches, false keeps them
        #[arg(
            short = 'f',
            long = "filter_outdated",
            value_name = "BOOL",
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        filter_outdated: Option<bool>,
    },

    /// Download the launcher pack
    Launcher {
        /// Region to download, all if unset
        #[arg(short = 'r', long = "region", value_name = "REGION")]
        region: Option<String>,

        /// Download every file in the manifest
        #[arg(
            short = 'a',
            long = "all",
            conflicts_with_all = ["pattern", "file"],
            default_value_t = false
        )]
        all: bool,

        /// Regex filter on resource name, repeatable
        #[arg(short = 'p', long = "pattern", value_name = "REGEX")]
        pattern: Vec<String>,

        /// Download only the listed files, repeatable
        #[arg(short = 'f', long = "file", value_name = "NAME")]
        file: Vec<String>,

        /// Enable regex filter, false uses FILES
        #[arg(
            short = 'e',
            long = "regex",
            value_name = "BOOL",
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        regex: Option<bool>,

        /// Save by manifest path, not by file name
        #[arg(short = 'k', long = "keep", default_value_t = false)]
        keep: bool,
    },

    /// Download the uncensor pack
    Uncensor {
        /// Region to download from
        #[arg(
            short = 'r',
            long = "region",
            value_name = "REGION",
            conflicts_with = "default"
        )]
        region: Option<String>,

        /// Download from the default URL
        #[arg(short = 'd', long = "default", default_value_t = false)]
        default: bool,

        /// Download from a custom file list
        #[arg(
            short = 'c',
            long = "custom",
            requires = "path",
            conflicts_with = "default",
            default_value_t = false
        )]
        custom: bool,

        /// Save resource and patch manifest json, false skips
        #[arg(
            short = 'm',
            long = "manifest",
            value_name = "BOOL",
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        manifest: Option<bool>,

        /// Path to the custom list JSON
        #[arg(short = 'p', long = "path", value_name = "PATH", requires = "custom")]
        path: Option<PathBuf>,
    },
}
