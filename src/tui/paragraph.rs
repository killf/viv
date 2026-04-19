use crate::core::terminal::buffer::{Buffer, Rect, char_width};
use crate::core::terminal::style::Color;
use crate::tui::widget::Widget;

/// A styled text segment within a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

impl Span {
    pub fn raw(text: impl Into<String>) -> Self {
        Span {
            text: text.into(),
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            dim: false,
        }
    }

    pub fn styled(text: impl Into<String>, fg: Color, bold: bool) -> Self {
        Span {
            text: text.into(),
            fg: Some(fg),
            bg: None,
            bold,
            italic: false,
            dim: false,
        }
    }
}

/// A single logical line composed of one or more spans.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn raw(text: impl Into<String>) -> Self {
        Line {
            spans: vec![Span::raw(text)],
        }
    }

    pub fn from_spans(spans: Vec<Span>) -> Self {
        Line { spans }
    }
}

/// A scrollable, word-wrapping block of text.
pub struct Paragraph {
    pub lines: Vec<Line>,
    pub scroll: u16,
}

impl Paragraph {
    pub fn new(lines: Vec<Line>) -> Self {
        Paragraph { lines, scroll: 0 }
    }

    pub fn scroll(mut self, offset: u16) -> Self {
        self.scroll = offset;
        self
    }
}

/// A single rendered character with its styling.
pub struct StyledChar {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub width: u16,
}

/// Word-wrap a logical `Line` into physical rows fitting within `width` columns.
pub fn wrap_line(line: &Line, width: usize) -> Vec<Vec<StyledChar>> {
    if width == 0 {
        return vec![];
    }

    let mut chars: Vec<StyledChar> = Vec::new();
    for span in &line.spans {
        for ch in span.text.chars() {
            chars.push(StyledChar {
                ch,
                fg: span.fg,
                bg: span.bg,
                bold: span.bold,
                italic: span.italic,
                dim: span.dim,
                width: char_width(ch),
            });
        }
    }

    if chars.is_empty() {
        return vec![vec![]];
    }

    let mut physical_lines: Vec<Vec<StyledChar>> = Vec::new();
    let mut current_row: Vec<StyledChar> = Vec::new();
    let mut current_width: usize = 0;

    let mut i = 0;
    while i < chars.len() {
        // Collect a word (non-space chars)
        let mut word: Vec<StyledChar> = Vec::new();
        let mut word_width: usize = 0;
        while i < chars.len() && chars[i].ch != ' ' {
            let w = chars[i].width as usize;
            word_width += w;
            word.push(StyledChar {
                ch: chars[i].ch,
                fg: chars[i].fg,
                bg: chars[i].bg,
                bold: chars[i].bold,
                italic: chars[i].italic,
                dim: chars[i].dim,
                width: chars[i].width,
            });
            i += 1;
        }
        // Collect trailing spaces
        let mut spaces: Vec<StyledChar> = Vec::new();
        while i < chars.len() && chars[i].ch == ' ' {
            spaces.push(StyledChar {
                ch: ' ',
                fg: chars[i].fg,
                bg: chars[i].bg,
                bold: chars[i].bold,
                italic: chars[i].italic,
                dim: chars[i].dim,
                width: 1,
            });
            i += 1;
        }

        if word.is_empty() {
            for sc in spaces {
                if current_width >= width {
                    physical_lines.push(current_row);
                    current_row = Vec::new();
                    current_width = 0;
                }
                current_width += sc.width as usize;
                current_row.push(sc);
            }
            continue;
        }

        // Try to fit the word
        if current_width + word_width <= width {
            // Fits on current row
            current_width += word_width;
            current_row.extend(word);
        } else if current_width == 0 {
            // Word alone on a new row — hard-break if too wide
            for sc in word {
                let w = sc.width as usize;
                if current_width + w > width && current_width > 0 {
                    physical_lines.push(current_row);
                    current_row = Vec::new();
                    current_width = 0;
                }
                current_width += w;
                current_row.push(sc);
            }
        } else {
            // Doesn't fit — wrap to next row
            physical_lines.push(current_row);
            current_row = Vec::new();
            current_width = 0;
            // Re-process: fit the word on the new row
            for sc in word {
                let w = sc.width as usize;
                if current_width + w > width && current_width > 0 {
                    physical_lines.push(current_row);
                    current_row = Vec::new();
                    current_width = 0;
                }
                current_width += w;
                current_row.push(sc);
            }
        }

        // Append trailing spaces
        for sc in spaces {
            if current_width >= width {
                physical_lines.push(current_row);
                current_row = Vec::new();
                current_width = 0;
            }
            current_width += 1;
            current_row.push(sc);
        }
    }

    if !current_row.is_empty() || physical_lines.is_empty() {
        physical_lines.push(current_row);
    }

    physical_lines
}

impl Widget for Paragraph {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        let mut all_rows: Vec<Vec<StyledChar>> = Vec::new();
        for line in &self.lines {
            all_rows.extend(wrap_line(line, width));
        }

        let start = self.scroll as usize;
        let visible = all_rows.iter().skip(start).take(height);

        for (row_idx, row) in visible.enumerate() {
            let y = area.y + row_idx as u16;
            let mut col = area.x;
            for sc in row {
                if sc.width == 0 {
                    continue;
                }
                if col + sc.width > area.x + area.width {
                    break;
                }
                let cell = buf.get_mut(col, y);
                cell.ch = sc.ch;
                cell.fg = sc.fg;
                cell.bg = sc.bg;
                cell.bold = sc.bold;
                cell.italic = sc.italic;
                cell.dim = sc.dim;
                if sc.width == 2 && col + 1 < area.x + area.width {
                    let cell2 = buf.get_mut(col + 1, y);
                    cell2.ch = '\0';
                    cell2.fg = sc.fg;
                    cell2.bg = sc.bg;
                    cell2.bold = sc.bold;
                    cell2.italic = sc.italic;
                    cell2.dim = sc.dim;
                }
                col += sc.width;
            }
        }
    }
}
