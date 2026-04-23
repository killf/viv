//! Serialize a `Buffer` row range into an ANSI byte stream suitable for
//! scrollback commits. Each row emits SGR transitions, character bytes, and
//! terminates with `ESC[0m\r\n`. Trailing blank cells are trimmed so short
//! lines don't pad out to the buffer width.
//!
//! The `\r` matters: we write these bytes to a terminal that's in raw mode
//! (OPOST disabled), so a bare `\n` moves the cursor down without returning
//! to column 0 — producing a staircase effect. `\r\n` restores the line
//! feed + carriage return pairing that cooked mode would have provided.

use std::ops::Range;

use crate::core::terminal::buffer::Buffer;
use crate::core::terminal::style::Color;

/// Serialize a row range of `buf` as ANSI bytes. Each row ends with `ESC[0m\r\n`.
/// Trailing blank cells (' ' or wide-char continuation '\0') are trimmed.
pub fn buffer_rows_to_ansi(buf: &Buffer, rows: Range<u16>) -> Vec<u8> {
    let width = buf.area.width;
    let row_count = rows.end.saturating_sub(rows.start) as usize;
    let mut out = Vec::with_capacity(row_count * width as usize);

    for y in rows {
        let mut last_fg: Option<Color> = None;
        let mut last_bold = false;

        // Find last non-blank column (exclusive end).
        let mut end_x = 0u16;
        for x in 0..width {
            let cell = buf.get(x, y);
            if cell.ch != ' ' && cell.ch != '\0' {
                end_x = x + 1;
            }
        }

        for x in 0..end_x {
            let cell = buf.get(x, y);
            if cell.ch == '\0' {
                // Wide-char continuation placeholder — primary cell already
                // wrote the glyph.
                continue;
            }
            if cell.fg != last_fg || cell.bold != last_bold {
                out.extend_from_slice(b"\x1b[0m");
                if cell.bold {
                    out.extend_from_slice(b"\x1b[1m");
                }
                if let Some(c) = cell.fg {
                    out.extend_from_slice(sgr_fg(c).as_bytes());
                }
                last_fg = cell.fg;
                last_bold = cell.bold;
            }
            let mut buf4 = [0u8; 4];
            let s = cell.ch.encode_utf8(&mut buf4);
            out.extend_from_slice(s.as_bytes());
        }
        out.extend_from_slice(b"\x1b[0m\r\n");
    }
    out
}

/// Emit the SGR foreground sequence for `c`.
fn sgr_fg(c: Color) -> String {
    match c {
        Color::Ansi(n) => format!("\x1b[{}m", n),
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
    }
}
