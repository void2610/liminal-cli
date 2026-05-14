use anstream::println;
use anstyle::{AnsiColor, Color, Style};

pub(crate) struct Logger {}

impl Logger {
    const GREEN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    const DIM: Style = Style::new().dimmed();

    pub(crate) fn print_ok(url: &str) {
        let g = Self::GREEN;
        println!("{g}ok{g:#}  {}", url);
    }

    pub(crate) fn dim(s: &str) {
        let d = Self::DIM;
        println!("{d}{}{d:#}", s);
    }
}
