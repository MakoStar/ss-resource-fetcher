use clap::builder::{
    Styles,
    styling::{Color, RgbColor, Style},
};
use owo_colors::Style as OwoStyle;

pub const HEADER_PREFIX: &str = "-- ";

pub const HEADER_STYLE: OwoStyle = OwoStyle::new().truecolor(51, 204, 255).bold();
pub const LEGEND_STYLE: OwoStyle = OwoStyle::new().truecolor(255, 215, 0).bold();
pub const TITLE_STYLE: OwoStyle = OwoStyle::new().truecolor(255, 255, 255).bold();
pub const NOTE_STYLE: OwoStyle = OwoStyle::new().truecolor(255, 102, 255);
pub const KEY_STYLE: OwoStyle = OwoStyle::new().truecolor(153, 255, 255);
pub const TYPE_STYLE: OwoStyle = OwoStyle::new().truecolor(150, 150, 150);
pub const VALUE_STYLE: OwoStyle = OwoStyle::new().truecolor(102, 255, 153);

pub fn help_styles() -> Styles {
    Styles::styled()
        .usage(
            Style::new()
                .bold()
                .fg_color(Some(Color::Rgb(RgbColor(255, 255, 255)))),
        )
        .header(
            Style::new()
                .bold()
                .fg_color(Some(Color::Rgb(RgbColor(51, 204, 255)))),
        )
        .literal(Style::new().fg_color(Some(Color::Rgb(RgbColor(102, 255, 153)))))
        .placeholder(Style::new().fg_color(Some(Color::Rgb(RgbColor(255, 215, 0)))))
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Rgb(RgbColor(255, 102, 102)))),
        )
        .invalid(Style::new().fg_color(Some(Color::Rgb(RgbColor(255, 102, 102)))))
}

pub fn resolve_color_choice() -> clap::ColorChoice {
    if std::env::var_os("NO_COLOR").is_some() {
        return clap::ColorChoice::Never;
    }

    if env_is_true("FORCE_COLOR") || env_is_true("CLICOLOR_FORCE") {
        return clap::ColorChoice::Always;
    }

    clap::ColorChoice::Auto
}

fn env_is_true(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => value != "0" && !value.eq_ignore_ascii_case("false"),
        Err(_) => false,
    }
}
