use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Plain,
    Rounded,
}

/// Which sides of the block to render borders on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl BorderSides {
    /// All four sides (default for a full box).
    pub const ALL: BorderSides = BorderSides {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };

    /// Only top and bottom (Claude Code input box style).
    pub const HORIZONTAL: BorderSides = BorderSides {
        top: true,
        right: false,
        bottom: true,
        left: false,
    };

    /// Only left and right.
    pub const VERTICAL: BorderSides = BorderSides {
        top: false,
        right: true,
        bottom: false,
        left: true,
    };
}

pub struct Block {
    pub title: Option<String>,
    pub border: BorderStyle,
    pub sides: BorderSides,
    pub border_fg: Option<Color>,
    pub title_fg: Option<Color>,
}

impl Block {
    pub fn new() -> Self {
        Block {
            title: None,
            border: BorderStyle::None,
            sides: BorderSides::ALL,
            border_fg: None,
            title_fg: None,
        }
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn border(mut self, b: BorderStyle) -> Self {
        self.border = b;
        self
    }

    /// Restrict which sides draw borders.
    pub fn borders(mut self, sides: BorderSides) -> Self {
        self.sides = sides;
        self
    }

    /// Set the border color.
    pub fn border_fg(mut self, fg: Color) -> Self {
        self.border_fg = Some(fg);
        self
    }

    /// Set the title color.
    pub fn title_fg(mut self, fg: Color) -> Self {
        self.title_fg = Some(fg);
        self
    }

    /// Returns the interior area after accounting for borders.
    pub fn inner(&self, area: Rect) -> Rect {
        if matches!(self.border, BorderStyle::None) {
            return area;
        }
        let top = if self.sides.top { 1 } else { 0 };
        let bottom = if self.sides.bottom { 1 } else { 0 };
        let left = if self.sides.left { 1 } else { 0 };
        let right = if self.sides.right { 1 } else { 0 };
        let width = area.width.saturating_sub(left + right);
        let height = area.height.saturating_sub(top + bottom);
        Rect::new(area.x + left, area.y + top, width, height)
    }
}

impl Default for Block {
    fn default() -> Self {
        Block::new()
    }
}

impl Widget for Block {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if matches!(self.border, BorderStyle::None) {
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
        let fg = self.border_fg;

        let put = |buf: &mut Buffer, cx: u16, cy: u16, ch: char| {
            let cell = buf.get_mut(cx, cy);
            cell.ch = ch;
            cell.fg = fg;
            cell.bold = false;
        };

        // Top row
        if self.sides.top {
            // Left-most char: corner if left side too, else horizontal line
            put(buf, x, y, if self.sides.left { top_left } else { '─' });
            for col in (x + 1)..right {
                put(buf, col, y, '─');
            }
            if area.width > 1 {
                put(
                    buf,
                    right,
                    y,
                    if self.sides.right { top_right } else { '─' },
                );
            }
        }

        // Bottom row
        if self.sides.bottom && area.height > 1 {
            put(
                buf,
                x,
                bottom,
                if self.sides.left { bottom_left } else { '─' },
            );
            for col in (x + 1)..right {
                put(buf, col, bottom, '─');
            }
            if area.width > 1 {
                put(
                    buf,
                    right,
                    bottom,
                    if self.sides.right {
                        bottom_right
                    } else {
                        '─'
                    },
                );
            }
        }

        // Side columns
        let side_start = if self.sides.top { y + 1 } else { y };
        let side_end = if self.sides.bottom {
            bottom
        } else {
            bottom + 1
        };
        if self.sides.left {
            for row in side_start..side_end {
                put(buf, x, row, '│');
            }
        }
        if self.sides.right && area.width > 1 {
            for row in side_start..side_end {
                put(buf, right, row, '│');
            }
        }

        // Title rendered on the top border row, starting at column offset 1
        if let Some(ref title) = self.title
            && self.sides.top
        {
            let title_x = x + 1;
            let max_x = right;
            let tfg = self.title_fg.or(self.border_fg);
            for (i, ch) in title.chars().enumerate() {
                let cur_x = title_x + i as u16;
                if cur_x >= max_x {
                    break;
                }
                let cell = buf.get_mut(cur_x, y);
                cell.ch = ch;
                cell.fg = tfg;
                cell.bold = false;
            }
        }
    }
}
