use anstyle::{AnsiColor, Color, Style};

pub const GREEN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
pub const RED: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
pub const YELLOW: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
pub const CYAN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
pub const DIM: Style = Style::new().dimmed();
pub const BOLD: Style = Style::new().bold();
