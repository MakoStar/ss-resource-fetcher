use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Write comments into the generated config
    #[arg(long = "comments", default_value_t = false)]
    pub comments: bool,

    /// Generate config: unset=if missing, true=regenerate, false=skip
    #[arg(
        long = "init",
        value_name = "BOOL",
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true
    )]
    pub init: Option<bool>,

    /// Override a config item, repeatable
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", global = true)]
    pub set: Vec<String>,

    #[command(subcommand)]
    pub task: Option<Task>,
}

#[derive(Subcommand, Debug)]
pub enum Task {
    /// Resource pipeline, default when no subcommand
    Run {
        /// Only dump the manifest record
        #[arg(short = 'r', long = "record", default_value_t = false)]
        record: bool,
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

        /// Path to the custom list JSON
        #[arg(short = 'p', long = "path", value_name = "PATH", requires = "custom")]
        path: Option<PathBuf>,
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
}
