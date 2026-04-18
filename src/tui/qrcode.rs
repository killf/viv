use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::qrcode::encode::select_version;
use crate::tui::widget::Widget;

const QUIET_ZONE: usize = 2;
const BLACK: Color = Color::Rgb(0, 0, 0);
const WHITE: Color = Color::Rgb(255, 255, 255);

pub struct QrCodeWidget<'a> {
    data: &'a str,
}

impl<'a> QrCodeWidget<'a> {
    pub fn new(data: &'a str) -> Self {
        QrCodeWidget { data }
    }

    /// Calculate rendered terminal height for the given data string.
    ///
    /// Uses `select_version` to predict QR version without full encoding.
    /// Falls back to version 1 if data length is 0 or out of range.
    pub fn height(data: &str) -> u16 {
        let version = select_version(data.len()).unwrap_or(1) as usize;
        let matrix_size = 4 * version + 17;
        let total_with_quiet = matrix_size + 2 * QUIET_ZONE;
        ((total_with_quiet + 1) / 2) as u16
    }
}

impl Widget for QrCodeWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Encode the QR code; on failure render error message
        let matrix = match crate::qrcode::encode(self.data) {
            Ok(m) => m,
            Err(_) => {
                let msg = "QR error";
                let x = area.x;
                let y = area.y;
                buf.set_str(x, y, msg, Some(Color::Rgb(171, 43, 63)), false);
                return;
            }
        };

        let mat_size = matrix.size();
        let total_width = mat_size + 2 * QUIET_ZONE;
        let total_rows = mat_size + 2 * QUIET_ZONE;
        // Each terminal row covers 2 QR rows
        let term_height = ((total_rows + 1) / 2) as u16;
        let term_width = total_width as u16;

        // Don't render if area is too small
        if area.width < term_width || area.height < term_height {
            // Still try to render what fits, or just return if truly tiny
            if area.width < 2 || area.height < 1 {
                return;
            }
        }

        // Center in available area
        let offset_x = if area.width > term_width {
            (area.width - term_width) / 2
        } else {
            0
        };
        let offset_y = if area.height > term_height {
            (area.height - term_height) / 2
        } else {
            0
        };

        let start_x = area.x + offset_x;
        let start_y = area.y + offset_y;

        // Helper: get module color (true=black, false=white) with quiet zone
        let get_module = |qr_row: isize, qr_col: isize| -> Color {
            let r = qr_row - QUIET_ZONE as isize;
            let c = qr_col - QUIET_ZONE as isize;
            if r < 0 || c < 0 || r >= mat_size as isize || c >= mat_size as isize {
                WHITE
            } else if matrix.get(r as usize, c as usize) {
                BLACK
            } else {
                WHITE
            }
        };

        let render_rows = term_height.min(area.height - offset_y);
        let render_cols = term_width.min(area.width - offset_x);

        for term_row in 0..render_rows {
            let top_qr_row = (term_row as usize) * 2;
            let bot_qr_row = top_qr_row + 1;

            for term_col in 0..render_cols {
                let qr_col = term_col as isize;

                let top_color = get_module(top_qr_row as isize, qr_col);
                let bot_color = if bot_qr_row < total_rows {
                    get_module(bot_qr_row as isize, qr_col)
                } else {
                    WHITE
                };

                let x = start_x + term_col;
                let y = start_y + term_row;

                let cell = buf.get_mut(x, y);
                cell.ch = '▀';
                cell.fg = Some(top_color);
                cell.bg = Some(bot_color);
            }
        }
    }
}
