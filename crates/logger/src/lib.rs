use std::fmt::Arguments;
use std::io::Result;

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
    let style: Style = level_style(record.level());
    let tag: &str = level_tag(record.level());

    eprintln!(
        "[{}][{}] - {}",
        ts.to_string().style(TS_STYLE),
        tag.style(style),
        record.args().to_string().style(style),
    );

    Ok(())
}

pub fn init(default_level: LevelFilter) {
    // Builder::from_default_env()
    //     .filter_level(default_level)
    //     .format(format_record)
    //     .init();
    let mut builder: Builder = Builder::new();

    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        builder.parse_filters(&rust_log);
    } else {
        builder.filter_level(default_level);
    }

    builder.format(format_record).init();
}

#[derive(Clone, Copy)]
pub struct CustomTag {
    pub name: &'static str,
    pub color: Style,
    pub bold: bool,
}

// 成功 - rgb(102, 255, 102)
pub const TAG_SUCC: CustomTag = CustomTag {
    name: "SUCC",
    color: Style::new().truecolor(102, 255, 102),
    bold: false,
};

// 失败 - rgb(255, 153, 102)
pub const TAG_FAIL: CustomTag = CustomTag {
    name: "FAIL",
    color: Style::new().truecolor(255, 153, 102),
    bold: false,
};

// 步骤 - rgb(153, 255, 255)
pub const TAG_STEP: CustomTag = CustomTag {
    name: "STEP",
    color: Style::new().truecolor(153, 255, 255),
    bold: false,
};

// 提示 - rgb(255, 102, 255)
pub const TAG_TIPS: CustomTag = CustomTag {
    name: "TIPS",
    color: Style::new().truecolor(255, 102, 255),
    bold: false,
};

// 节点 - rgb(102, 102, 255)
pub const TAG_NODE: CustomTag = CustomTag {
    name: "NODE",
    color: Style::new().truecolor(102, 102, 255),
    bold: false,
};

// 测试 - rgb(204, 204, 204)
pub const TAG_TEST: CustomTag = CustomTag {
    name: "TEST",
    // color: 250,
    color: Style::new().truecolor(204, 204, 204),
    bold: true,
};

#[doc(hidden)]
pub fn custom_log(tag: &CustomTag, msg: Arguments) {
    let ts: DelayedFormat<StrftimeItems<'_>> = Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut style: Style = tag.color;
    if tag.bold {
        style = style.bold();
    }

    eprintln!(
        "[{}][{}] - {}",
        ts.to_string().style(TS_STYLE),
        tag.name.style(style),
        msg.style(style),
    );
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
        $crate::init(log::LevelFilter::Debug);
    };
    ($level:expr) => {
        $crate::set_env_color();
        $crate::init($level)
    };
}

// #[macro_export]
// macro_rules! log_tag {
//     ($tag:expr, $($arg:tt)*) => {
//         $crate::custom_log(
//             &$tag,
//             format_args!($($arg)*)
//         )
//     };
// }

#[macro_export]
macro_rules! head {
    ($fmt:expr $(, $args:expr)* $(,)?) => {{
        let msg = format!($fmt $(, $args)*);
        let total_width: usize = 64;
        let msg_width = $crate::unicode_width::UnicodeWidthStr::width(msg.as_str());
        let overhead = 2usize;
        let pad = total_width.saturating_sub(msg_width + overhead).max(2);
        let left = pad / 2;
        let right = pad - left;
        $crate::node!("{} {} {}", "═".repeat(left), msg, "═".repeat(right));
    }};
}

#[macro_export]
macro_rules! succ {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_SUCC,
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! fail {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_FAIL,
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_STEP,
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! tips {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_TIPS,
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! node {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_NODE,
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! test {
    ($($arg:tt)*) => {
        $crate::custom_log(
            &$crate::TAG_TEST,
            format_args!($($arg)*)
        )
    };
}
