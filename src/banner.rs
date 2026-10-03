use unicode_width::UnicodeWidthStr;

#[macro_export]
macro_rules! print_banner {
    () => {
        $crate::banner::print_banner(env!("CARGO_BIN_NAME"))
    };
    ($t:expr) => {
        $crate::banner::print_banner($t)
    };
}

pub fn print_banner(t: impl AsRef<str>) {
    let title = format!(
        "             StellaSora    v{}    {}         ",
        env!("CARGO_PKG_VERSION"),
        t.as_ref(),
    );
    print_boxed(&[
        title.as_str(),
        "               (╯°□°)╯     ₍^..^₎      ╰(°□°╰)             ",
    ]);
}

fn print_boxed(lines: &[&str]) {
    let max_width = lines.iter().map(|line| line.width()).max().unwrap_or(0);
    let inner_width = max_width + 2;

    logger::tips!("{}", format!("╔{}╗", "═".repeat(inner_width)));
    for line in lines {
        let padding = inner_width - line.width() - 2;
        logger::tips!("║ {}{} ║", line, " ".repeat(padding));
    }
    logger::tips!("{}", format!("╚{}╝", "═".repeat(inner_width)));
}
