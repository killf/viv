use crate::terminal::output::AnsiWriter;

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
    pub fg: Option<u8>,
    pub bg: Option<u8>,
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
    pub fn set_str(&mut self, x: u16, y: u16, s: &str, fg: Option<u8>, bold: bool) {
        let max_x = self.area.x + self.area.width;
        let mut cur_x = x;
        for ch in s.chars() {
            if cur_x >= max_x {
                break;
            }
            let i = self.idx(cur_x, y);
            self.cells[i] = Cell { ch, fg, bg: None, bold };
            cur_x += 1;
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
            let col = (i % width) as u16 + self.area.x;
            let row = (i / width) as u16 + self.area.y;

            writer.move_to(row, col);

            let cell = &self.cells[i];

            // Apply background color if present
            if let Some(bg) = cell.bg {
                let seq = format!("\x1b[{}m", bg + 10);
                writer.write_bytes(seq.as_bytes());
            }

            // Apply foreground color if present
            if let Some(fg) = cell.fg {
                let seq = format!("\x1b[{}m", fg);
                writer.write_bytes(seq.as_bytes());
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
