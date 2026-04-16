use crate::terminal::buffer::{Buffer, Rect};
use crate::tui::widget::Widget;

/// A styled text segment within a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub fg: Option<u8>,
    pub bold: bool,
}

impl Span {
    /// Create an unstyled span.
    pub fn raw(text: impl Into<String>) -> Self {
        Span { text: text.into(), fg: None, bold: false }
    }

    /// Create a styled span with a foreground color and optional bold.
    pub fn styled(text: impl Into<String>, fg: u8, bold: bool) -> Self {
        Span { text: text.into(), fg: Some(fg), bold }
    }
}

/// A single logical line composed of one or more spans.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    /// Create a line from a single unstyled string.
    pub fn raw(text: impl Into<String>) -> Self {
        Line { spans: vec![Span::raw(text)] }
    }

    /// Create a line from an explicit list of spans.
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

    /// Builder method to set the vertical scroll offset (in logical lines).
    pub fn scroll(mut self, offset: u16) -> Self {
        self.scroll = offset;
        self
    }
}

/// A single rendered character with its styling attributes.
type StyledChar = (char, Option<u8>, bool);

/// Word-wrap a logical `Line` into physical rows, each at most `width` chars wide.
///
/// Breaking rules:
/// - Break at spaces between words.
/// - If a single word is longer than `width`, hard-break mid-word.
fn wrap_line(line: &Line, width: usize) -> Vec<Vec<StyledChar>> {
    if width == 0 {
        return vec![];
    }

    // Flatten all spans into a sequence of styled chars.
    let mut chars: Vec<StyledChar> = Vec::new();
    for span in &line.spans {
        for ch in span.text.chars() {
            chars.push((ch, span.fg, span.bold));
        }
    }

    if chars.is_empty() {
        return vec![vec![]];
    }

    // Collect words: each word is a Vec<StyledChar> followed by any trailing spaces.
    // We preserve spaces as their own "word" tokens so styling is maintained.
    let mut physical_lines: Vec<Vec<StyledChar>> = Vec::new();
    let mut current_row: Vec<StyledChar> = Vec::new();

    // We process by splitting into word+space tokens.
    let mut i = 0;
    while i < chars.len() {
        // Collect a word (non-space chars).
        let mut word: Vec<StyledChar> = Vec::new();
        while i < chars.len() && chars[i].0 != ' ' {
            word.push(chars[i]);
            i += 1;
        }
        // Collect trailing spaces.
        let mut spaces: Vec<StyledChar> = Vec::new();
        while i < chars.len() && chars[i].0 == ' ' {
            spaces.push(chars[i]);
            i += 1;
        }

        if word.is_empty() {
            // Leading spaces or consecutive spaces: treat as part of current row.
            for sc in spaces {
                if current_row.len() >= width {
                    physical_lines.push(current_row);
                    current_row = Vec::new();
                }
                current_row.push(sc);
            }
            continue;
        }

        // Hard-break words that exceed `width` entirely.
        let mut remaining_word = word;
        while !remaining_word.is_empty() {
            let space_left = width.saturating_sub(current_row.len());
            if space_left == 0 {
                physical_lines.push(current_row);
                current_row = Vec::new();
                continue;
            }
            if remaining_word.len() <= space_left {
                // Fits entirely.
                current_row.extend_from_slice(&remaining_word);
                remaining_word = Vec::new();
            } else if current_row.is_empty() {
                // Word alone exceeds width; hard-break.
                current_row.extend_from_slice(&remaining_word[..space_left]);
                remaining_word = remaining_word[space_left..].to_vec();
                physical_lines.push(current_row);
                current_row = Vec::new();
            } else {
                // Word doesn't fit in remaining space; push to next row.
                physical_lines.push(current_row);
                current_row = Vec::new();
            }
        }

        // Append trailing spaces (they may wrap too).
        for sc in spaces {
            if current_row.len() >= width {
                physical_lines.push(current_row);
                current_row = Vec::new();
            }
            current_row.push(sc);
        }
    }

    if !current_row.is_empty() {
        physical_lines.push(current_row);
    }

    if physical_lines.is_empty() {
        physical_lines.push(vec![]);
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

        // Build all physical rows across all logical lines.
        let mut all_rows: Vec<Vec<StyledChar>> = Vec::new();
        for line in &self.lines {
            let wrapped = wrap_line(line, width);
            all_rows.extend(wrapped);
        }

        // Apply scroll offset and render up to `height` rows.
        let start = self.scroll as usize;
        let visible = all_rows.iter().skip(start).take(height);

        for (row_idx, row) in visible.enumerate() {
            let y = area.y + row_idx as u16;
            for (col_idx, &(ch, fg, bold)) in row.iter().enumerate() {
                let x = area.x + col_idx as u16;
                if x >= area.x + area.width {
                    break;
                }
                let cell = buf.get_mut(x, y);
                cell.ch = ch;
                cell.fg = fg;
                cell.bold = bold;
            }
        }
    }
}
