pub mod encode;
pub mod gf256;
pub mod matrix;
pub mod rs;
pub mod tables;

pub use matrix::QrMatrix;

use crate::core::terminal::buffer::{Buffer, Rect};
use crate::tui::widget::Widget;

/// Encode UTF-8 text into a QR code matrix.
pub fn encode(text: &str) -> crate::Result<QrMatrix> {
    let encoded = encode::encode_and_interleave(text)?;
    Ok(QrMatrix::build(encoded.version, &encoded.data))
}

/// A TUI widget that renders a QR code using half-block characters.
pub struct QrCodeWidget {
    text: String,
}

impl QrCodeWidget {
    pub fn new(text: impl Into<String>) -> Self {
        QrCodeWidget { text: text.into() }
    }

    /// Returns the number of terminal rows needed to display the QR code for `text`.
    pub fn height(text: &str) -> u16 {
        match crate::tui::qrcode::encode(text) {
            Ok(matrix) => {
                let quiet = 4u16;
                let size = matrix.size() as u16 + quiet * 2;
                (size + 1) / 2
            }
            Err(_) => 0,
        }
    }
}

impl Widget for QrCodeWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let matrix = match crate::tui::qrcode::encode(&self.text) {
            Ok(m) => m,
            Err(_) => return,
        };

        let quiet: usize = 4;
        let size = matrix.size() + quiet * 2;
        let rows_needed = (size + 1) / 2;
        let cols_needed = size;

        let area_w = area.width as usize;
        let area_h = area.height as usize;

        // Center horizontally and vertically
        let x_offset = if cols_needed < area_w {
            (area_w - cols_needed) / 2
        } else {
            0
        };
        let y_offset = if rows_needed < area_h {
            (area_h - rows_needed) / 2
        } else {
            0
        };

        let get_module = |row: isize, col: isize| -> bool {
            let r = row - quiet as isize;
            let c = col - quiet as isize;
            if r < 0 || c < 0 || r >= matrix.size() as isize || c >= matrix.size() as isize {
                false // quiet zone = white
            } else {
                matrix.get(r as usize, c as usize)
            }
        };

        for pair in 0..rows_needed {
            let top_row = (pair * 2) as isize;
            let bot_row = top_row + 1;

            let y = area.y + y_offset as u16 + pair as u16;
            if y >= area.y + area.height {
                break;
            }

            for col in 0..size.min(area_w) {
                let x = area.x + x_offset as u16 + col as u16;
                if x >= area.x + area.width {
                    break;
                }

                let top = get_module(top_row, col as isize);
                let bot = get_module(bot_row, col as isize);

                let cell = buf.get_mut(x, y);
                cell.ch = match (top, bot) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                };
            }
        }
    }
}
