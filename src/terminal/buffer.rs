use crate::terminal::output::AnsiWriter;
use crate::terminal::style::Color;

/// Returns the display width of a character in terminal columns.
/// CJK and fullwidth characters take 2 columns; most others take 1.
pub fn char_width(ch: char) -> u16 {
    let c = ch as u32;
    // Control characters
    if c < 0x20 || c == 0x7F {
        return 0;
    }
    // CJK Unified Ideographs, CJK Extension A/B, CJK Compatibility Ideographs
    if (0x2E80..=0x9FFF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0xFE30..=0xFE6F).contains(&c)      // CJK Compatibility Forms
        || (0xFF01..=0xFF60).contains(&c)       // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&c)       // Fullwidth Signs
        || (0x20000..=0x2FA1F).contains(&c)     // CJK Extension B-F + Supplements
        || (0x30000..=0x3134F).contains(&c)     // CJK Extension G
    {
        return 2;
    }
    // Hangul Syllables
    if (0xAC00..=0xD7AF).contains(&c) {
        return 2;
    }
    // Katakana/Hiragana/Bopomofo etc.
    if (0x3000..=0x303F).contains(&c)           // CJK Symbols and Punctuation
        || (0x3040..=0x309F).contains(&c)       // Hiragana
        || (0x30A0..=0x30FF).contains(&c)       // Katakana
        || (0x3100..=0x312F).contains(&c)       // Bopomofo
        || (0x3130..=0x318F).contains(&c)       // Hangul Compatibility Jamo
        || (0x31F0..=0x31FF).contains(&c)       // Katakana Phonetic Extensions
    {
        return 2;
    }
    1
}

/// A rectangular region of the terminal, described by position and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Rect { x, y, width, height }
    }

    /// Total number of cells in the rect.
    pub fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Split into left and right halves at column offset (relative to self.x).
    /// The left rect has width = offset; the right rect gets the remainder.
    pub fn split_horizontal(self, offset: u16) -> (Rect, Rect) {
        let left_width = offset.min(self.width);
        let right_width = self.width.saturating_sub(left_width);
        let left = Rect::new(self.x, self.y, left_width, self.height);
        let right = Rect::new(self.x + left_width, self.y, right_width, self.height);
        (left, right)
    }

    /// Split into top and bottom halves at row offset (relative to self.y).
    /// The top rect has height = offset; the bottom rect gets the remainder.
    pub fn split_vertical(self, offset: u16) -> (Rect, Rect) {
        let top_height = offset.min(self.height);
        let bottom_height = self.height.saturating_sub(top_height);
        let top = Rect::new(self.x, self.y, self.width, top_height);
        let bottom = Rect::new(self.x, self.y + top_height, self.width, bottom_height);
        (top, bottom)
    }

    /// Shrink this rect by 1 on all sides. Width/height floor at 0.
    pub fn inner(self) -> Rect {
        let x = self.x.saturating_add(1);
        let y = self.y.saturating_add(1);
        let width = self.width.saturating_sub(2);
        let height = self.height.saturating_sub(2);
        Rect::new(x, y, width, height)
    }
}

/// A single terminal cell with character and optional styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', fg: None, bg: None, bold: false }
    }
}

/// A flat buffer of cells covering a rectangular area.
#[derive(Clone)]
pub struct Buffer {
    pub area: Rect,
    cells: Vec<Cell>,
}

impl Buffer {
    /// Create a buffer filled with default (blank) cells.
    pub fn empty(area: Rect) -> Self {
        let size = area.area() as usize;
        Buffer { area, cells: vec![Cell::default(); size] }
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        let col = (x.saturating_sub(self.area.x)) as usize;
        let row = (y.saturating_sub(self.area.y)) as usize;
        row * self.area.width as usize + col
    }

    pub fn get(&self, x: u16, y: u16) -> &Cell {
        &self.cells[self.idx(x, y)]
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let i = self.idx(x, y);
        &mut self.cells[i]
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        let i = self.idx(x, y);
        self.cells[i] = cell;
    }

    pub fn set_char(&mut self, x: u16, y: u16, ch: char) {
        self.get_mut(x, y).ch = ch;
    }

    /// Write a string starting at (x, y), clipping at the right edge of the buffer.
    /// Wide characters (CJK) occupy 2 cells; a placeholder '\0' is placed in the second cell.
    pub fn set_str(&mut self, x: u16, y: u16, s: &str, fg: Option<Color>, bold: bool) {
        let max_x = self.area.x + self.area.width;
        let mut cur_x = x;
        for ch in s.chars() {
            let w = char_width(ch);
            if w == 0 { continue; }
            if cur_x + w > max_x {
                break;
            }
            let i = self.idx(cur_x, y);
            self.cells[i] = Cell { ch, fg, bg: None, bold };
            // For wide chars, fill the next cell with a placeholder
            if w == 2 && cur_x + 1 < max_x {
                let i2 = self.idx(cur_x + 1, y);
                self.cells[i2] = Cell { ch: '\0', fg, bg: None, bold };
            }
            cur_x += w;
        }
    }

    /// Reset all cells to default.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
    }

    /// Generate ANSI bytes for every cell that differs from `previous`.
    /// Both buffers must cover the same area.
    pub fn diff(&self, previous: &Buffer) -> Vec<u8> {
        let mut writer = AnsiWriter::new();
        let len = self.cells.len().min(previous.cells.len());
        let width = self.area.width as usize;

        for i in 0..len {
            if self.cells[i] == previous.cells[i] {
                continue;
            }
            let cell = &self.cells[i];
            // Skip wide-char placeholder cells — the primary cell handles rendering
            if cell.ch == '\0' {
                continue;
            }
            let col = (i % width) as u16 + self.area.x;
            let row = (i / width) as u16 + self.area.y;

            writer.move_to(row, col);

            let cell = &self.cells[i];

            // Apply background color if present
            if let Some(bg) = cell.bg {
                writer.write_bytes(bg.bg_seq().as_bytes());
            }

            // Apply foreground color if present
            if let Some(fg) = cell.fg {
                writer.write_bytes(fg.fg_seq().as_bytes());
            }

            // Apply bold if set
            if cell.bold {
                writer.bold();
            }

            // Write the character
            let mut char_buf = [0u8; 4];
            writer.write_bytes(cell.ch.encode_utf8(&mut char_buf).as_bytes());

            writer.reset_style();
        }

        writer.take()
    }
}
