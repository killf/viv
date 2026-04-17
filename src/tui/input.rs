use crate::core::terminal::buffer::{char_width, Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::widget::Widget;

/// A text input widget with a prompt, cursor, and multiline support.
pub struct InputWidget<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub prompt: &'a str,
    pub prompt_fg: Option<Color>,
    pub placeholder: Option<&'a str>,
}

impl<'a> InputWidget<'a> {
    pub fn new(content: &'a str, cursor: usize, prompt: &'a str) -> Self {
        InputWidget { content, cursor, prompt, prompt_fg: None, placeholder: None }
    }

    /// Builder: set prompt foreground color.
    pub fn prompt_fg(mut self, fg: Color) -> Self {
        self.prompt_fg = Some(fg);
        self
    }

    /// Builder: set placeholder text (shown when content is empty).
    pub fn placeholder(mut self, text: Option<&'a str>) -> Self {
        self.placeholder = text;
        self
    }

    /// Returns the absolute (col, row) position where the cursor should be placed.
    pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
        let prompt_width: u16 = self.prompt.chars().map(char_width).sum();
        let before = &self.content[..self.cursor.min(self.content.len())];
        let cursor_row = before.chars().filter(|&c| c == '\n').count() as u16;
        let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let cursor_col: u16 = before[last_nl..].chars().map(char_width).sum();
        (area.x + prompt_width + cursor_col, area.y + cursor_row)
    }
}

impl Widget for InputWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // If content is empty and placeholder is set, render prompt + placeholder
        if self.content.is_empty() {
            if let Some(ph) = self.placeholder {
                let prompt_chars: Vec<char> = self.prompt.chars().collect();
                let ph_chars: Vec<char> = ph.chars().collect();
                let mut col = area.x;
                for (ch, is_prompt) in prompt_chars.iter().map(|&c| (c, true))
                    .chain(ph_chars.iter().map(|&c| (c, false)))
                {
                    let w = char_width(ch) as usize;
                    if w == 0 { continue; }
                    if col + w as u16 > area.x + area.width { break; }
                    let fg = if is_prompt {
                        self.prompt_fg
                    } else {
                        Some(crate::core::terminal::style::theme::DIM)
                    };
                    let cell = buf.get_mut(col, area.y);
                    cell.ch = ch;
                    cell.fg = fg;
                    cell.bold = false;
                    col += w as u16;
                }
                return;
            }
        }

        let prompt_width: u16 = self.prompt.chars().map(char_width).sum();
        let logical_lines: Vec<&str> = self.content.split('\n').collect();

        for (row_idx, line) in logical_lines.iter().enumerate() {
            let y = area.y + row_idx as u16;
            if y >= area.y + area.height { break; }

            let mut col = area.x;
            if row_idx == 0 {
                // First row: render prompt then line content
                for ch in self.prompt.chars() {
                    let w = char_width(ch);
                    if col + w > area.x + area.width { break; }
                    let cell = buf.get_mut(col, y);
                    cell.ch = ch; cell.fg = self.prompt_fg; cell.bold = false;
                    col += w;
                }
            } else {
                // Continuation rows: indent by prompt_width (no visible prompt chars)
                col = area.x + prompt_width;
            }

            for ch in line.chars() {
                let w = char_width(ch);
                if w == 0 { continue; }
                if col + w > area.x + area.width { break; }
                let cell = buf.get_mut(col, y);
                cell.ch = ch; cell.fg = None; cell.bold = false;
                if w == 2 && col + 1 < area.x + area.width {
                    let cell2 = buf.get_mut(col + 1, y);
                    cell2.ch = '\0'; cell2.fg = None; cell2.bold = false;
                }
                col += w;
            }
        }
    }
}
