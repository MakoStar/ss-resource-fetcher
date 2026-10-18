use std::fmt::Arguments;
use std::io::Result;

#[doc(hidden)]
pub use log;

#[doc(hidden)]
pub use unicode_width;

use chrono::Local;
use chrono::format::{DelayedFormat, StrftimeItems};
use env_logger::{Builder, fmt::Formatter};
use log::{Level, LevelFilter, Record};
use owo_colors::{OwoColorize, Style};

const TS_STYLE: Style = Style::new().truecolor(51, 153, 51);

fn level_style(level: Level) -> Style {
    match level {
        Level::Trace => Style::new().truecolor(114, 115, 116),
        Level::Debug => Style::new().truecolor(114, 115, 116),
        Level::Info => Style::new().truecolor(102, 255, 153),
        Level::Warn => Style::new().truecolor(255, 255, 102),
        Level::Error => Style::new().truecolor(255, 102, 102),
    }
}

fn level_tag(level: Level) -> &'static str {
    match level {
        Level::Error => "ERRO",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBU",
        Level::Trace => "TRAC",
    }
}

#[allow(unused)]
fn format_record(buf: &mut Formatter, record: &Record<'_>) -> Result<()> {
    let ts: DelayedFormat<StrftimeItems<'_>> = Local::now().format("%Y-%m-%d %H:%M:%S");
    let style = level_style(record.level());
    let tag: &str = level_tag(record.level());

    let line = format!(
        "[{}][{}] - {}",
        ts.to_string().style(TS_STYLE),
        tag.style(style),
        record.args().to_string().style(style),
    );

    eprintln!("{line}");

    let _ = buf;

    Ok(())
}

#[doc(hidden)]
pub fn allows(level: LevelFilter) -> bool {
    level != LevelFilter::Off && level <= log::max_level()
}

pub fn init(default_level: LevelFilter) {
    let mut builder = Builder::new();

    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        builder.parse_filters(&rust_log);
    } else {
        builder.filter_level(default_level);
    }

    let _ = builder.format(format_record).try_init();
}

#[derive(Clone, Copy)]
pub struct CustomTag {
    pub name: &'static str,
    pub color: Style,
    pub bold: bool,
}

pub const TAG_SUCC: CustomTag = CustomTag {
    name: "SUCC",
    color: Style::new().truecolor(102, 255, 102),
    bold: false,
};

pub const TAG_FAIL: CustomTag = CustomTag {
    name: "FAIL",
    color: Style::new().truecolor(255, 153, 102),
    bold: false,
};

pub const TAG_TIER: CustomTag = CustomTag {
    name: "TIER",
    color: Style::new().truecolor(51, 204, 255),
    bold: true,
};

pub const TAG_STEP: CustomTag = CustomTag {
    name: "STEP",
    color: Style::new().truecolor(153, 255, 255),
    bold: false,
};

pub const TAG_TIPS: CustomTag = CustomTag {
    name: "TIPS",
    color: Style::new().truecolor(255, 102, 255),
    bold: false,
};

pub const TAG_NODE: CustomTag = CustomTag {
    name: "NODE",
    color: Style::new().truecolor(102, 102, 255),
    bold: false,
};

pub const TAG_TEST: CustomTag = CustomTag {
    name: "TEST",
    color: Style::new().truecolor(204, 204, 204),
    bold: true,
};

#[doc(hidden)]
pub fn custom_log(tag: &CustomTag, msg: Arguments) {
    let ts: DelayedFormat<StrftimeItems<'_>> = Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut style = tag.color;
    if tag.bold {
        style = style.bold();
    }

    let line = format!(
        "[{}][{}] - {}",
        ts.to_string().style(TS_STYLE),
        tag.name.style(style),
        msg.to_string().style(style),
    );

    eprintln!("{line}");
}

pub fn set_env_color() {
    use owo_colors;
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        owo_colors::set_override(true);
    }
}

#[macro_export]
macro_rules! init_logger {
    () => {
        $crate::set_env_color();
        $crate::init(log::LevelFilter::Info);
    };
    ($level:expr) => {
        $crate::set_env_color();
        $crate::init($level)
    };
}

#[macro_export]
macro_rules! head {
    ($fmt:expr $(, $args:expr)* $(,)?) => {{
        if $crate::allows($crate::log::LevelFilter::Info) {
            let msg = format!($fmt $(, $args)*);
            let total_width = 64usize;
            let msg_width = $crate::unicode_width::UnicodeWidthStr::width(msg.as_str());
            let overhead = 2usize;
            let pad = total_width.saturating_sub(msg_width + overhead).max(2);
            let left = pad / 2;
            let right = pad - left;
            $crate::node!("{} {} {}", "═".repeat(left), msg, "═".repeat(right));
        }
    }};
}

#[macro_export]
macro_rules! tier {
    ($fmt:expr $(, $args:expr)* $(,)?) => {{
        if $crate::allows($crate::log::LevelFilter::Info) {
            let msg = format!($fmt $(, $args)*);
            let width = $crate::unicode_width::UnicodeWidthStr::width(msg.as_str());
            let fill = 60usize.saturating_sub(width).max(2);
            $crate::custom_log(
                &$crate::TAG_TIER,
                format_args!("── {} {}", msg, "─".repeat(fill)),
            );
        }
    }};
}

#[macro_export]
macro_rules! succ {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Info) {
            $crate::custom_log(&$crate::TAG_SUCC, format_args!($($arg)*))
        }
    };
}

#[macro_export]
macro_rules! fail {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Error) {
            $crate::custom_log(&$crate::TAG_FAIL, format_args!($($arg)*))
        }
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Info) {
            $crate::custom_log(&$crate::TAG_STEP, format_args!($($arg)*))
        }
    };
}

#[macro_export]
macro_rules! tips {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Info) {
            $crate::custom_log(&$crate::TAG_TIPS, format_args!($($arg)*))
        }
    };
}

#[macro_export]
macro_rules! node {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Info) {
            $crate::custom_log(&$crate::TAG_NODE, format_args!($($arg)*))
        }
    };
}

#[macro_export]
macro_rules! test {
    ($($arg:tt)*) => {
        if $crate::allows($crate::log::LevelFilter::Trace) {
            $crate::custom_log(&$crate::TAG_TEST, format_args!($($arg)*))
        }
    };
}
