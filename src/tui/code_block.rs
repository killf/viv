use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::block::{Block, BorderStyle};
use crate::tui::syntax::{TokenKind, tokenize};
use crate::tui::widget::Widget;

pub struct CodeBlockWidget<'a> {
    code: &'a str,
    language: Option<&'a str>,
}

impl<'a> CodeBlockWidget<'a> {
    pub fn new(code: &'a str, language: Option<&'a str>) -> Self {
        CodeBlockWidget { code, language }
    }

    /// Returns the height needed to render this code block (line count + 2 for borders).
    pub fn height(code: &str, _width: u16) -> u16 {
        let line_count = if code.is_empty() {
            1
        } else {
            code.lines().count()
        };
        line_count as u16 + 2
    }

    fn token_color(kind: TokenKind) -> (Color, bool) {
        match kind {
            TokenKind::Keyword => (Color::Rgb(110, 150, 255), true),
            TokenKind::String => (Color::Rgb(120, 200, 120), false),
            TokenKind::Comment => (Color::Rgb(100, 100, 100), false),
            TokenKind::Number => (Color::Rgb(215, 160, 87), false),
            TokenKind::Type => (Color::Rgb(100, 200, 200), false),
            TokenKind::Function => (Color::Rgb(230, 220, 110), false),
            TokenKind::Operator => (Color::Rgb(200, 200, 200), false),
            TokenKind::Punctuation => (Color::Rgb(150, 150, 150), false),
            TokenKind::Attribute => (Color::Rgb(180, 130, 230), false),
            TokenKind::Lifetime => (Color::Rgb(200, 150, 100), false),
            TokenKind::Plain => (Color::Rgb(220, 220, 220), false),
        }
    }
}

impl<'a> Widget for CodeBlockWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Too small to render anything useful
        if area.width < 4 || area.height < 3 {
            return;
        }

        // Build and render the border block
        let mut block = Block::new()
            .border(BorderStyle::Rounded)
            .border_fg(Color::Rgb(80, 80, 80));

        if let Some(lang) = self.language {
            let title = format!(" {} ", lang);
            block = block.title(title).title_fg(Color::Rgb(150, 150, 150));
        }

        block.render(area, buf);

        // Get inner area (inside the border)
        let inner = block.inner(area);
        if inner.is_empty() {
            return;
        }

        // No background fill — keep terminal default background

        // Render each line of code
        let lines: Vec<&str> = if self.code.is_empty() {
            vec![""]
        } else {
            self.code.lines().collect()
        };

        for (row_idx, line) in lines.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let tokens = tokenize(line, self.language);
            let mut x = inner.x;
            let max_x = inner.x + inner.width;

            for token in tokens {
                if x >= max_x {
                    break;
                }
                let (fg, bold) = Self::token_color(token.kind);
                for ch in token.text.chars() {
                    if x >= max_x {
                        break;
                    }
                    let cell = buf.get_mut(x, y);
                    cell.ch = ch;
                    cell.fg = Some(fg);
                    cell.bg = None;
                    cell.bold = bold;
                    x += 1;
                }
            }
        }
    }
}
