//! Color type supporting ANSI palette and 24-bit RGB, plus the Claude Code theme.

/// A terminal color — either an ANSI palette index (30-37, 90-97, etc.)
/// or a 24-bit RGB triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// ANSI SGR color code (e.g. 31 for red, 90 for bright-black/dim).
    Ansi(u8),
    /// 24-bit truecolor RGB.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Produce the ANSI escape sequence to set this color as foreground.
    pub fn fg_seq(&self) -> String {
        match self {
            Color::Ansi(n) => format!("\x1b[{}m", n),
            Color::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
        }
    }

    /// Produce the ANSI escape sequence to set this color as background.
    pub fn bg_seq(&self) -> String {
        match self {
            Color::Ansi(n) => format!("\x1b[{}m", n + 10),
            Color::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
        }
    }
}

/// Claude Code default-dark theme colors (pulled from
/// `claude-code-restored/src/utils/theme.ts`).
pub mod theme {
    use super::Color;

    /// Claude orange — main brand color for assistant messages.
    pub const CLAUDE: Color = Color::Rgb(215, 119, 87);

    /// Lighter shimmer tone for Claude branding.
    pub const CLAUDE_SHIMMER: Color = Color::Rgb(235, 159, 127);

    /// Dim gray — prompt border, hint text, secondary content.
    pub const DIM: Color = Color::Rgb(136, 136, 136);

    /// Primary text — white.
    pub const TEXT: Color = Color::Rgb(255, 255, 255);

    /// Light blue-purple — suggestions and permissions.
    pub const SUGGESTION: Color = Color::Rgb(177, 185, 249);

    /// Green — success indicators.
    pub const SUCCESS: Color = Color::Rgb(78, 186, 101);

    /// Red — errors and stalled states.
    pub const ERROR: Color = Color::Rgb(171, 43, 63);
}
