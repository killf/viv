use crate::terminal::buffer::{Buffer, Rect};
use crate::tui::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Plain,
    Rounded,
}

pub struct Block {
    pub title: Option<String>,
    pub border: BorderStyle,
}

impl Block {
    pub fn new() -> Self {
        Block { title: None, border: BorderStyle::None }
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn border(mut self, b: BorderStyle) -> Self {
        self.border = b;
        self
    }

    /// Returns the interior area after accounting for borders.
    pub fn inner(&self, area: Rect) -> Rect {
        match self.border {
            BorderStyle::None => area,
            BorderStyle::Plain | BorderStyle::Rounded => area.inner(),
        }
    }
}

impl Default for Block {
    fn default() -> Self {
        Block::new()
    }
}

impl Widget for Block {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if self.border == BorderStyle::None {
            return;
        }
        if area.is_empty() {
            return;
        }

        let (top_left, top_right, bottom_left, bottom_right) = match self.border {
            BorderStyle::Plain => ('┌', '┐', '└', '┘'),
            BorderStyle::Rounded => ('╭', '╮', '╰', '╯'),
            BorderStyle::None => return,
        };

        let x = area.x;
        let y = area.y;
        let right = area.x + area.width.saturating_sub(1);
        let bottom = area.y + area.height.saturating_sub(1);

        // Draw corners
        buf.set_char(x, y, top_left);
        buf.set_char(right, y, top_right);
        buf.set_char(x, bottom, bottom_left);
        buf.set_char(right, bottom, bottom_right);

        // Draw top and bottom horizontal lines
        for col in (x + 1)..right {
            buf.set_char(col, y, '─');
            buf.set_char(col, bottom, '─');
        }

        // Draw left and right vertical lines
        for row in (y + 1)..bottom {
            buf.set_char(x, row, '│');
            buf.set_char(right, row, '│');
        }

        // Render title on top border row starting at column 1
        if let Some(ref title) = self.title {
            let title_x = x + 1;
            let max_x = right;
            let mut cur_x = title_x;
            for ch in title.chars() {
                if cur_x >= max_x {
                    break;
                }
                buf.set_char(cur_x, y, ch);
                cur_x += 1;
            }
        }
    }
}
