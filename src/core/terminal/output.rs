use std::fmt;

/// ANSI color codes for foreground colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black   = 30,
    Red     = 31,
    Green   = 32,
    Yellow  = 33,
    Blue    = 34,
    Magenta = 35,
    Cyan    = 36,
    White   = 37,
}

/// A buffered writer that produces ANSI escape sequences.
pub struct AnsiWriter {
    buf: Vec<u8>,
}

impl AnsiWriter {
    pub fn new() -> Self {
        AnsiWriter { buf: Vec::new() }
    }

    /// Returns the buffered bytes and clears the internal buffer.
    pub fn take(&mut self) -> Vec<u8> {
        let out = self.buf.clone();
        self.buf.clear();
        out
    }

    /// Move cursor to (row, col), converting from 0-indexed to 1-indexed.
    pub fn move_to(&mut self, row: u16, col: u16) {
        let seq = format!("\x1b[{};{}H", row + 1, col + 1);
        self.buf.extend_from_slice(seq.as_bytes());
    }

    /// Clear the entire screen.
    pub fn clear_screen(&mut self) {
        self.buf.extend_from_slice(b"\x1b[2J");
    }

    /// Clear the current line.
    pub fn clear_line(&mut self) {
        self.buf.extend_from_slice(b"\x1b[2K");
    }

    /// Set foreground color.
    pub fn fg_color(&mut self, color: Color) {
        let seq = format!("\x1b[{}m", color as u8);
        self.buf.extend_from_slice(seq.as_bytes());
    }

    /// Enable bold text.
    pub fn bold(&mut self) {
        self.buf.extend_from_slice(b"\x1b[1m");
    }

    /// Reset all text styles.
    pub fn reset_style(&mut self) {
        self.buf.extend_from_slice(b"\x1b[0m");
    }

    /// Hide the cursor.
    pub fn hide_cursor(&mut self) {
        self.buf.extend_from_slice(b"\x1b[?25l");
    }

    /// Show the cursor.
    pub fn show_cursor(&mut self) {
        self.buf.extend_from_slice(b"\x1b[?25h");
    }

    /// Append a string's bytes to the buffer.
    pub fn write_str(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// Append raw bytes to the buffer.
    pub fn write_bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
}

impl Default for AnsiWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Write for AnsiWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.buf.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
