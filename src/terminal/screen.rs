use crate::terminal::output::AnsiWriter;

/// A single character cell with optional styling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<u8>,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', fg: None, bold: false }
    }
}

/// Double-buffered terminal screen.
pub struct Screen {
    pub width: u16,
    pub height: u16,
    front: Vec<Cell>,
    back: Vec<Cell>,
}

impl Screen {
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Screen {
            width,
            height,
            front: vec![Cell::default(); size],
            back: vec![Cell::default(); size],
        }
    }

    fn idx(&self, row: u16, col: u16) -> usize {
        (row as usize) * (self.width as usize) + (col as usize)
    }

    /// Read a cell from the back buffer.
    pub fn get(&self, row: u16, col: u16) -> Cell {
        self.back[self.idx(row, col)]
    }

    /// Write a character to the back buffer (no styling).
    pub fn put(&mut self, row: u16, col: u16, ch: char) {
        let i = self.idx(row, col);
        self.back[i].ch = ch;
        self.back[i].fg = None;
        self.back[i].bold = false;
    }

    /// Write a styled character to the back buffer.
    pub fn put_styled(&mut self, row: u16, col: u16, ch: char, fg: Option<u8>, bold: bool) {
        let i = self.idx(row, col);
        self.back[i] = Cell { ch, fg, bold };
    }

    /// Write a string of characters to the back buffer starting at (row, col).
    pub fn put_str(&mut self, row: u16, col: u16, s: &str) {
        for (offset, ch) in s.chars().enumerate() {
            let c = col as usize + offset;
            if c >= self.width as usize {
                break;
            }
            let i = (row as usize) * (self.width as usize) + c;
            self.back[i] = Cell { ch, fg: None, bold: false };
        }
    }

    /// Reset the back buffer to all default cells.
    pub fn clear_back(&mut self) {
        for cell in self.back.iter_mut() {
            *cell = Cell::default();
        }
    }

    /// Compare back vs front, generate ANSI bytes for changed cells,
    /// then synchronise front = back.
    pub fn diff(&mut self) -> Vec<u8> {
        let mut writer = AnsiWriter::new();

        for row in 0..self.height {
            for col in 0..self.width {
                let i = self.idx(row, col);
                let back_cell = self.back[i];
                let front_cell = self.front[i];

                if back_cell == front_cell {
                    continue;
                }

                // Move cursor to this cell position.
                writer.move_to(row, col);

                let styled = back_cell.bold || back_cell.fg.is_some();

                if back_cell.bold {
                    writer.bold();
                }
                if let Some(fg) = back_cell.fg {
                    let seq = format!("\x1b[{}m", fg);
                    writer.write_bytes(seq.as_bytes());
                }

                // Write the character.
                let mut buf = [0u8; 4];
                let s = back_cell.ch.encode_utf8(&mut buf);
                writer.write_str(s);

                if styled {
                    writer.reset_style();
                }
            }
        }

        // Sync: copy back buffer into front buffer.
        self.front.copy_from_slice(&self.back);

        writer.take()
    }
}
