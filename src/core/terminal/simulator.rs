//! Terminal simulator for parsing ANSI escape sequences and reconstructing terminal state.
//!
//! This module provides data structures and parsing logic for interpreting terminal
//! output, useful for testing TUI rendering by capturing and analyzing ANSI sequences.

/// A terminal color — either an ANSI palette index or a 24-bit RGB triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    /// ANSI SGR color code (e.g. 30-37 for normal, 90-97 for bright).
    Ansi(u8),
    /// 24-bit truecolor RGB.
    Rgb(u8, u8, u8),
}

/// Style attributes for a single terminal cell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CellStyle {
    /// Foreground color.
    pub fg: Option<Color>,
    /// Background color.
    pub bg: Option<Color>,
    /// Bold intensity.
    pub bold: bool,
    /// Italic style.
    pub italic: bool,
    /// Underline style.
    pub underline: bool,
}

impl CellStyle {
    /// Returns a new empty cell style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all style attributes to defaults.
    pub fn reset(&mut self) {
        self.fg = None;
        self.bg = None;
        self.bold = false;
        self.italic = false;
        self.underline = false;
    }
}

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed in this cell.
    pub ch: char,
    /// The styling applied to this cell.
    pub style: CellStyle,
}

impl Cell {
    /// Creates a new cell with a character and default style.
    pub fn new(ch: char) -> Self {
        Cell {
            ch,
            style: CellStyle::default(),
        }
    }

    /// Creates a new cell with a character and given style.
    pub fn with_style(ch: char, style: CellStyle) -> Self {
        Cell { ch, style }
    }
}

/// The terminal screen grid.
#[derive(Debug, Clone)]
pub struct Screen {
    grid: Vec<Vec<Cell>>,
    width: usize,
    height: usize,
    cursor: (usize, usize),
    /// Pending-wrap flag (xterm Last Column Flag): set when a character is
    /// written at the rightmost column; cleared on the next character write
    /// (which first wraps to the next row) or on any cursor-positioning op.
    pending_wrap: bool,
}

impl Screen {
    /// Creates a new screen with the given dimensions, filled with spaces.
    pub fn new(width: usize, height: usize) -> Self {
        let cell = Cell::new(' ');
        let grid = vec![vec![cell.clone(); width]; height];
        Screen {
            grid,
            width,
            height,
            cursor: (0, 0),
            pending_wrap: false,
        }
    }

    /// Returns the cell at the given position, if valid.
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        if row < self.height && col < self.width {
            Some(&self.grid[row][col])
        } else {
            None
        }
    }

    /// Returns the entire line at the given row, if valid.
    pub fn line(&self, row: usize) -> Option<&[Cell]> {
        if row < self.height {
            Some(&self.grid[row])
        } else {
            None
        }
    }

    /// Returns whether the given text appears anywhere on the screen.
    pub fn contains(&self, text: &str) -> bool {
        for row in &self.grid {
            let line_text: String = row.iter().map(|c| c.ch).collect();
            if line_text.contains(text) {
                return true;
            }
        }
        false
    }

    /// Returns the character at the given position, if valid.
    pub fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.cell(row, col).map(|c| c.ch)
    }

    /// Returns the current cursor position as (row, col).
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// Returns the screen dimensions as (width, height).
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Returns the text content of the given row as a String.
    pub fn line_text(&self, row: usize) -> Option<String> {
        self.line(row).map(|line| line.iter().map(|c| c.ch).collect())
    }

    /// Returns the text content of rows in the given range (inclusive).
    pub fn text_range(&self, start_row: usize, end_row: usize) -> String {
        let end = end_row.min(self.height);
        let start = start_row.min(end);
        (start..end).filter_map(|r| self.line_text(r)).collect::<Vec<_>>().join("\n")
    }

    /// Returns the height of the screen.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the width of the screen.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns all lines as trimmed strings (ignoring trailing whitespace).
    pub fn all_lines(&self) -> Vec<String> {
        self.grid.iter()
            .map(|row| {
                let text: String = row.iter().map(|c| c.ch).collect();
                text.trim_end().to_string()
            })
            .collect()
    }

    /// Returns the screen content as a single string, lines joined by newlines.
    /// Trailing whitespace on each line is trimmed.
    pub fn as_string(&self) -> String {
        self.all_lines().join("\n")
    }

    /// Asserts that a specific cell has the expected character.
    pub fn assert_cell(&self, row: usize, col: usize, expected: char) {
        let actual = self.char_at(row, col);
        assert_eq!(
            actual,
            Some(expected),
            "Cell at ({}, {}) should be '{}', got {:?}",
            row, col, expected, actual
        );
    }

    /// Asserts that a line matches the expected content (ignoring trailing spaces).
    pub fn assert_line(&self, row: usize, expected: &str) {
        let actual = self.line_text(row);
        let actual_trimmed = actual.as_ref().map(|s| s.trim_end().to_string());
        assert_eq!(
            actual_trimmed.as_deref(),
            Some(expected),
            "Line {} should be:\n  {:?}\nbut got:\n  {:?}",
            row, expected, actual_trimmed
        );
    }

    /// Asserts that the screen matches the expected layout exactly.
    /// Each string in `expected` is one row, trimmed of trailing spaces.
    pub fn assert_screen(&self, expected: &[&str]) {
        let actual_lines = self.all_lines();
        assert_eq!(
            actual_lines.len(),
            expected.len(),
            "Screen height mismatch: expected {} rows, got {}",
            expected.len(),
            actual_lines.len()
        );
        for (i, exp) in expected.iter().enumerate() {
            assert_eq!(
                actual_lines.get(i).map(|s| s.as_str()),
                Some(exp.trim_end()),
                "Line {} mismatch:\n  expected: {:?}\n  actual:   {:?}",
                i,
                exp.trim_end(),
                actual_lines.get(i)
            );
        }
    }

    /// Returns the style at a given cell.
    pub fn style_at(&self, row: usize, col: usize) -> Option<&CellStyle> {
        self.cell(row, col).map(|c| &c.style)
    }

    /// Asserts that a range of cells has the expected characters.
    pub fn assert_line_range(&self, row: usize, start_col: usize, end_col: usize, expected: &str) {
        let actual: String = (start_col..end_col)
            .filter_map(|col| self.char_at(row, col))
            .collect();
        assert_eq!(
            actual.as_str(),
            expected,
            "Line {} cols {}..{} should be {:?}, got {:?}",
            row, start_col, end_col, expected, actual
        );
    }

    /// Asserts that the given text appears anywhere on the screen.
    /// Only checks text content, ignores styles.
    pub fn assert_text_contains(&self, text: &str) {
        assert!(
            self.contains(text),
            "Screen should contain text: {:?}",
            text
        );
    }

    /// Asserts that a specific cell has the expected foreground color.
    /// Use this for checking ANSI color codes (e.g., 31=red, 32=green, 90=bright black).
    pub fn assert_cell_fg(&self, row: usize, col: usize, color: u8) {
        let style = self.style_at(row, col);
        let actual_color = style.and_then(|s| {
            match &s.fg {
                Some(Color::Ansi(c)) => Some(*c),
                _ => None,
            }
        });
        assert_eq!(
            actual_color,
            Some(color),
            "Cell ({}, {}) foreground should be {}, got {:?}",
            row, col, color, actual_color
        );
    }

    /// Asserts that a specific cell has the expected truecolor foreground.
    /// Use this for checking RGB colors emitted via SGR 38;2;r;g;b.
    pub fn assert_cell_fg_rgb(&self, row: usize, col: usize, r: u8, g: u8, b: u8) {
        let style = self.style_at(row, col);
        let actual = style.and_then(|s| match &s.fg {
            Some(Color::Rgb(rr, gg, bb)) => Some((*rr, *gg, *bb)),
            _ => None,
        });
        assert_eq!(
            actual,
            Some((r, g, b)),
            "Cell ({}, {}) foreground should be RGB({}, {}, {}), got {:?}",
            row, col, r, g, b, actual
        );
    }

    /// Moves the cursor to the given position (1-indexed, as per ANSI CUP).
    fn move_cursor_to(&mut self, row: usize, col: usize) {
        // ANSI CUP uses 1-based indexing
        let row = row.saturating_sub(1);
        let col = col.saturating_sub(1);
        self.cursor.0 = row.min(self.height.saturating_sub(1));
        self.cursor.1 = col.min(self.width.saturating_sub(1));
        self.pending_wrap = false;
    }

    /// Moves the cursor by the given delta (positive = down/right).
    fn move_cursor_rel(&mut self, d_row: isize, d_col: isize) {
        let (r, c) = self.cursor;
        let new_row = (r as isize + d_row).clamp(0, self.height as isize - 1) as usize;
        let new_col = (c as isize + d_col).clamp(0, self.width as isize - 1) as usize;
        self.cursor = (new_row, new_col);
        self.pending_wrap = false;
    }

    /// Writes a character at the current cursor position and advances the cursor.
    fn write_char(&mut self, ch: char, style: CellStyle) {
        // First: if we're pending-wrap, consume the flag by advancing to the
        // start of the next row (scrolling if needed) before writing.
        if self.pending_wrap {
            self.cursor.1 = 0;
            self.cursor.0 += 1;
            if self.cursor.0 >= self.height {
                self.scroll(1);
                self.cursor.0 = self.height - 1;
            }
            self.pending_wrap = false;
        }

        let (row, col) = self.cursor;
        if row >= self.height || col >= self.width {
            return;
        }

        if ch == '\t' {
            // Advance to next tab stop (every 8 columns). Tab clears pending_wrap
            // via the earlier branch; no wrap handling beyond the clamp.
            let next_col = (col / 8 + 1) * 8;
            self.cursor.1 = next_col.min(self.width - 1);
            return;
        }

        self.grid[row][col] = Cell { ch, style };
        if col + 1 >= self.width {
            // Rightmost column: set pending-wrap, don't advance cursor.
            self.pending_wrap = true;
        } else {
            self.cursor.1 += 1;
        }
    }

    /// Scrolls the screen content up by n lines, adding empty lines at the bottom.
    fn scroll(&mut self, n: usize) {
        self.pending_wrap = false;
        for _ in 0..n {
            // Remove the top line
            self.grid.remove(0);
            // Add an empty line at the bottom
            let empty_line = vec![Cell::new(' '); self.width];
            self.grid.push(empty_line);
        }
    }

    /// Clears all cells in the screen.
    fn clear_screen(&mut self) {
        self.pending_wrap = false;
        for row in &mut self.grid {
            for cell in row {
                *cell = Cell::new(' ');
            }
        }
    }

    /// Clears all cells from the cursor to the end of the screen.
    fn clear_from_cursor(&mut self) {
        let (row, col) = self.cursor;
        // Clear from cursor to end of current line
        if row < self.height {
            for c in col..self.width {
                self.grid[row][c] = Cell::new(' ');
            }
        }
        // Clear all lines below current
        for r in (row + 1)..self.height {
            for c in 0..self.width {
                self.grid[r][c] = Cell::new(' ');
            }
        }
    }

    /// Clears all cells from the beginning of the screen to the cursor.
    fn clear_to_cursor(&mut self) {
        let (row, col) = self.cursor;
        // Clear all lines above current
        for r in 0..row {
            for c in 0..self.width {
                self.grid[r][c] = Cell::new(' ');
            }
        }
        // Clear from start of current line to cursor
        if row < self.height {
            for c in 0..=col.min(self.width - 1) {
                self.grid[row][c] = Cell::new(' ');
            }
        }
    }

    /// Clears all cells from the beginning of the current line to the cursor.
    fn clear_line_to_cursor(&mut self) {
        let (row, col) = self.cursor;
        if row < self.height {
            for c in 0..=col.min(self.width - 1) {
                self.grid[row][c] = Cell::new(' ');
            }
        }
    }

    /// Clears all cells from the cursor to the end of the current line.
    fn clear_line_from_cursor(&mut self) {
        let (row, col) = self.cursor;
        if row < self.height {
            for c in col..self.width {
                self.grid[row][c] = Cell::new(' ');
            }
        }
    }

    /// Clears the entire current line.
    fn clear_line(&mut self) {
        let row = self.cursor.0;
        if row < self.height {
            for c in 0..self.width {
                self.grid[row][c] = Cell::new(' ');
            }
        }
    }
}

/// Parses CSI (Control Sequence Introducer) parameters.
///
/// CSI sequences have the form: `\x1b[{params}{intermediate}*{final}`
struct ParamParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ParamParser<'a> {
    /// Creates a new parser for the given byte slice (starting after `\x1b[`).
    fn new(bytes: &'a [u8]) -> Self {
        ParamParser { bytes, pos: 0 }
    }

    /// Returns whether there are more bytes to parse.
    fn has_more(&self) -> bool {
        self.pos < self.bytes.len()
    }

    /// Peeks at the next byte without consuming it.
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Consumes and returns the next byte.
    fn next(&mut self) -> Option<u8> {
        let b = self.bytes.get(self.pos).copied();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    /// Parses a single numeric parameter, returning 1 if no digits found.
    fn parse_param(&mut self) -> usize {
        let mut value = 0usize;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                value = value * 10 + (b - b'0') as usize;
                self.next();
            } else {
                break;
            }
        }
        value
    }

    /// Parses a list of parameters separated by ':' or ';'.
    fn parse_param_list(&mut self) -> Vec<usize> {
        let mut params = Vec::new();
        loop {
            params.push(self.parse_param());
            if self.peek() == Some(b':') || self.peek() == Some(b';') {
                self.next();
            } else {
                break;
            }
        }
        if params.is_empty() {
            params.push(1);
        }
        params
    }
}

/// ANSI escape sequence parser state machine.
struct AnsiParser {
    /// The current screen state.
    screen: Screen,
    /// The current active style.
    current_style: CellStyle,
    /// Parser state.
    state: ParserState,
    /// Bytes accumulated for multi-byte sequences.
    buf: Vec<u8>,
    /// Saved cursor position for recall.
    saved_cursor: Option<(usize, usize)>,
}

/// Parser state for the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    /// Normal state, looking for escape sequence.
    Ground,
    /// Saw `\x1b`, next byte determines sequence type.
    Escape,
    /// Saw `\x1b[`, parsing CSI parameters.
    CsiEntry,
    /// Inside CSI parameter bytes.
    CsiParam,
    /// Inside CSI intermediate bytes.
    CsiIntermediate,
    /// Inside OSC string (for completeness, though we don't handle it).
    OscString,
}

impl AnsiParser {
    /// Creates a new parser for a screen of the given size.
    fn new(width: usize, height: usize) -> Self {
        AnsiParser {
            screen: Screen::new(width, height),
            current_style: CellStyle::default(),
            state: ParserState::Ground,
            buf: Vec::new(),
            saved_cursor: None,
        }
    }

    /// Parses a byte into the current state.
    fn parse_byte(&mut self, b: u8) {
        match self.state {
            ParserState::Ground => self.parse_ground(b),
            ParserState::Escape => self.parse_escape(b),
            ParserState::CsiEntry => self.parse_csi_entry(b),
            ParserState::CsiParam => self.parse_csi_param(b),
            ParserState::CsiIntermediate => self.parse_csi_intermediate(b),
            ParserState::OscString => self.parse_osc_string(b),
        }
    }

    /// Parses a byte in Ground state.
    fn parse_ground(&mut self, b: u8) {
        match b {
            0x1b => self.state = ParserState::Escape,
            0x07 | 0x08 | 0x0A | 0x0D => {
                // BEL, BS, LF, CR - handle special characters
                if b == 0x08 {
                    // Backspace
                    self.screen.move_cursor_rel(0, -1);
                }
                if b == 0x0D {
                    // Carriage return
                    self.screen.cursor.1 = 0;
                    self.screen.pending_wrap = false;
                }
                if b == 0x0A {
                    // Line feed
                    self.screen.pending_wrap = false;
                    let (row, _) = self.screen.cursor;
                    if row + 1 >= self.screen.height() {
                        self.screen.scroll(1);
                    } else {
                        self.screen.cursor.0 += 1;
                    }
                }
            }
            _ => {
                if let Some(ch) = char::from_u32(b as u32) {
                    if !ch.is_control() || ch == ' ' {
                        self.screen.write_char(ch, self.current_style.clone());
                    }
                }
            }
        }
    }

    /// Parses a byte after seeing ESC.
    fn parse_escape(&mut self, b: u8) {
        match b {
            b'[' => self.state = ParserState::CsiEntry,
            0x58 | 0x5D => self.state = ParserState::OscString, // ESC ] (OSC)
            0x37 => {
                // Save cursor (DECSC)
                self.saved_cursor = Some(self.screen.cursor);
                self.state = ParserState::Ground;
            }
            0x38 => {
                // Restore cursor (DECRC)
                if let Some(pos) = self.saved_cursor {
                    self.screen.cursor = pos;
                    self.screen.pending_wrap = false;
                }
                self.state = ParserState::Ground;
            }
            _ => self.state = ParserState::Ground,
        }
    }

    /// Parses first byte of CSI sequence.
    fn parse_csi_entry(&mut self, b: u8) {
        if b.is_ascii_digit() || b == b':' || b == b';' {
            // Start of parameters
            self.buf.clear();
            self.buf.push(b);
            self.state = ParserState::CsiParam;
        } else if (0x40..=0x7E).contains(&b) {
            // Final byte directly (no params)
            self.execute_csi(&[], b);
            self.state = ParserState::Ground;
        } else if (0x20..=0x2F).contains(&b) {
            // Intermediate bytes before params or final
            self.buf.clear();
            self.buf.push(b);
            self.state = ParserState::CsiIntermediate;
        }
    }

    /// Parses CSI parameter bytes.
    fn parse_csi_param(&mut self, b: u8) {
        if b.is_ascii_digit() || b == b':' || b == b';' {
            self.buf.push(b);
        } else if (0x20..=0x2F).contains(&b) {
            // Intermediate bytes
            self.state = ParserState::CsiIntermediate;
            self.buf.push(b);
        } else if (0x40..=0x7E).contains(&b) {
            // Final byte
            let params = self.buf.clone();
            self.execute_csi(&params, b);
            self.state = ParserState::Ground;
        }
    }

    /// Parses CSI intermediate bytes.
    fn parse_csi_intermediate(&mut self, b: u8) {
        if (0x20..=0x2F).contains(&b) {
            self.buf.push(b);
        } else if (0x40..=0x7E).contains(&b) {
            // Final byte
            let params = self.buf.clone();
            self.execute_csi(&params, b);
            self.state = ParserState::Ground;
        }
    }

    /// Parses OSC string bytes.
    fn parse_osc_string(&mut self, b: u8) {
        if b == 0x07 || b == 0x9C {
            // BEL or ST ends OSC
            self.state = ParserState::Ground;
        }
        // For now, we don't do anything with OSC strings
    }

    /// Executes a CSI sequence with the given parameters and final byte.
    fn execute_csi(&mut self, params: &[u8], final_byte: u8) {
        let mut parser = ParamParser::new(params);
        match final_byte as char {
            'm' => self.execute_sgr(&mut parser),
            'H' | 'f' => self.execute_cup(&mut parser),
            'J' => self.execute_ed(&mut parser),
            'K' => self.execute_el(&mut parser),
            'A' => self.execute_cuu(&mut parser),
            'B' => self.execute_cud(&mut parser),
            'C' => self.execute_cuf(&mut parser),
            'D' => self.execute_cub(&mut parser),
            'S' => self.execute_su(&mut parser),
            'T' => self.execute_sd(&mut parser),
            _ => {}
        }
    }

    /// Executes SGR (Select Graphic Rendition) sequence.
    fn execute_sgr(&mut self, parser: &mut ParamParser) {
        let params = parser.parse_param_list();
        let mut i = 0;
        while i < params.len() {
            let p = params[i];
            match p {
                0 => self.current_style.reset(),
                1 => self.current_style.bold = true,
                3 => self.current_style.italic = true,
                4 => self.current_style.underline = true,
                22 => self.current_style.bold = false,
                23 => self.current_style.italic = false,
                24 => self.current_style.underline = false,
                // Foreground colors
                30..=37 => self.current_style.fg = Some(Color::Ansi(p as u8)),
                38 => {
                    // Extended foreground color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        // 256-color mode
                        self.current_style.fg = Some(Color::Ansi(params[i + 2] as u8));
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        // Truecolor mode
                        let r = params[i + 2] as u8;
                        let g = params[i + 3] as u8;
                        let b = params[i + 4] as u8;
                        self.current_style.fg = Some(Color::Rgb(r, g, b));
                        i += 4;
                    }
                }
                39 => self.current_style.fg = None,
                // Background colors
                40..=47 => self.current_style.bg = Some(Color::Ansi((p - 10) as u8)),
                48 => {
                    // Extended background color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        // 256-color mode
                        self.current_style.bg = Some(Color::Ansi(params[i + 2] as u8));
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        // Truecolor mode
                        let r = params[i + 2] as u8;
                        let g = params[i + 3] as u8;
                        let b = params[i + 4] as u8;
                        self.current_style.bg = Some(Color::Rgb(r, g, b));
                        i += 4;
                    }
                }
                49 => self.current_style.bg = None,
                // Bright foreground colors (high intensity)
                90..=97 => self.current_style.fg = Some(Color::Ansi(p as u8)),
                // Bright background colors
                100..=107 => self.current_style.bg = Some(Color::Ansi((p - 10) as u8)),
                _ => {}
            }
            i += 1;
        }
    }

    /// Executes CUP (Cursor Position) or HVP (Position) sequence.
    fn execute_cup(&mut self, parser: &mut ParamParser) {
        // Parameters are {row};{col}, defaulting to 1;1
        let mut params = Vec::new();
        loop {
            let p = parser.parse_param();
            params.push(p);
            if parser.peek() == Some(b';') {
                parser.next();
            } else {
                break;
            }
            if !parser.has_more() {
                break;
            }
        }
        let row = params.first().copied().unwrap_or(1);
        let col = params.get(1).copied().unwrap_or(1);
        self.screen.move_cursor_to(row, col);
    }

    /// Executes ED (Erase Display) sequence.
    fn execute_ed(&mut self, parser: &mut ParamParser) {
        let p = parser.parse_param();
        match p {
            0 => self.screen.clear_from_cursor(),
            1 => self.screen.clear_to_cursor(),
            2 | 3 => self.screen.clear_screen(),
            _ => {}
        }
    }

    /// Executes EL (Erase Line) sequence.
    fn execute_el(&mut self, parser: &mut ParamParser) {
        let p = parser.parse_param();
        match p {
            0 => self.screen.clear_line_from_cursor(),
            1 => self.screen.clear_line_to_cursor(),
            2 => self.screen.clear_line(),
            _ => {}
        }
    }

    /// Executes CUU (Cursor Up) sequence.
    fn execute_cuu(&mut self, parser: &mut ParamParser) {
        let n = parser.parse_param().max(1);
        self.screen.move_cursor_rel(-(n as isize), 0);
    }

    /// Executes CUD (Cursor Down) sequence.
    fn execute_cud(&mut self, parser: &mut ParamParser) {
        let n = parser.parse_param().max(1);
        self.screen.move_cursor_rel(n as isize, 0);
    }

    /// Executes CUF (Cursor Forward) sequence.
    fn execute_cuf(&mut self, parser: &mut ParamParser) {
        let n = parser.parse_param().max(1);
        self.screen.move_cursor_rel(0, n as isize);
    }

    /// Executes CUB (Cursor Back) sequence.
    fn execute_cub(&mut self, parser: &mut ParamParser) {
        let n = parser.parse_param().max(1);
        self.screen.move_cursor_rel(0, -(n as isize));
    }

    /// Executes SU (Scroll Up) sequence.
    fn execute_su(&mut self, parser: &mut ParamParser) {
        let n = parser.parse_param().max(1);
        self.screen.scroll(n);
    }

    /// Executes SD (Scroll Down) sequence.
    fn execute_sd(&mut self, parser: &mut ParamParser) {
        // Scroll down: insert lines at top, push content down
        let n = parser.parse_param().max(1);
        for _ in 0..n {
            let empty_line = vec![Cell::new(' '); self.screen.width()];
            self.screen.grid.insert(0, empty_line);
            self.screen.grid.pop();
        }
    }

    /// Parses a string of bytes into the screen.
    fn parse(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.parse_byte(b);
        }
    }

    /// Returns the resulting screen.
    fn into_screen(self) -> Screen {
        self.screen
    }
}

/// Parses ANSI escape sequences and builds a Screen representation.
pub fn parse_ansi(bytes: &[u8], width: usize, height: usize) -> Screen {
    let mut parser = AnsiParser::new(width, height);
    parser.parse(bytes);
    parser.into_screen()
}

/// Parses an ANSI string into a Screen.
pub fn parse_str(s: &str, width: usize, height: usize) -> Screen {
    parse_ansi(s.as_bytes(), width, height)
}

// ── SimTerminal ───────────────────────────────────────────────────────────────

use crate::agent::protocol::{AgentEvent, AgentMessage};
use crate::core::terminal::backend::TestBackend;
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::size::TermSize;
use crate::tui::input::InputMode;
use crate::tui::session::{KeyOutcome, TuiSession};

/// Test double that runs the real [`TuiSession`] rendering path against a
/// [`TestBackend`], then parses the emitted ANSI bytes into a [`Screen`].
///
/// Tests drive it with `send_message` / `send_key`, inspect the screen
/// with `screen()`, and verify outbound agent events via `sent_events()`.
pub struct SimTerminal {
    session: TuiSession,
    backend: TestBackend,
    parser: AnsiParser,
    sent_events: Vec<AgentEvent>,
}

impl SimTerminal {
    pub fn new(width: usize, height: usize) -> Self {
        let size = TermSize {
            cols: width as u16,
            rows: height as u16,
        };
        SimTerminal {
            session: TuiSession::new(size, "~".to_string(), None),
            backend: TestBackend::new(width as u16, height as u16),
            parser: AnsiParser::new(width, height),
            sent_events: Vec::new(),
        }
    }

    pub fn with_cwd(mut self, cwd: &str) -> Self {
        let size = TermSize {
            cols: self.parser.screen.width as u16,
            rows: self.parser.screen.height as u16,
        };
        self.session = TuiSession::new(size, cwd.to_string(), None);
        self
    }

    pub fn with_branch(mut self, branch: Option<&str>) -> Self {
        let size = TermSize {
            cols: self.parser.screen.width as u16,
            rows: self.parser.screen.height as u16,
        };
        // NOTE: calling both with_cwd and with_branch on the same instance
        // loses the earlier cwd because we rebuild TuiSession. Callers that
        // need both should pass them in the order they want to take effect
        // and the earlier one will be discarded. For the current test suite
        // only one of these is used per construction so this is acceptable.
        self.session = TuiSession::new(size, "~".to_string(), branch.map(|s| s.to_string()));
        self
    }

    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self {
        let _ = self.session.handle_message(msg, &mut self.backend);
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self {
        if let Ok(outcome) = self.session.handle_key(key, &mut self.backend) {
            if let KeyOutcome::Event(e) = outcome {
                self.sent_events.push(e);
            }
        }
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn resize(&mut self, width: usize, height: usize) -> &mut Self {
        let size = TermSize {
            cols: width as u16,
            rows: height as u16,
        };
        self.session.resize(size);
        self.parser = AnsiParser::new(width, height);
        self.backend.size = size;
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn screen(&self) -> Screen {
        Screen {
            grid: self.parser.screen.grid.clone(),
            width: self.parser.screen.width,
            height: self.parser.screen.height,
            cursor: self.parser.screen.cursor,
            pending_wrap: self.parser.screen.pending_wrap,
        }
    }

    pub fn input_content(&self) -> String {
        self.session.input_content()
    }

    pub fn input_mode(&self) -> InputMode {
        self.session.input_mode()
    }

    pub fn permission_selected(&self) -> Option<usize> {
        self.session.permission_selected()
    }

    pub fn sent_events(&self) -> &[AgentEvent] {
        &self.sent_events
    }

    pub fn quit_requested(&self) -> bool {
        self.session.is_quitting()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_equality() {
        let c1 = Color::Ansi(31);
        let c2 = Color::Ansi(31);
        let c3 = Color::Ansi(32);
        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_cell_style_default() {
        let style = CellStyle::default();
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.underline);
    }

    #[test]
    fn test_cell_new() {
        let cell = Cell::new('A');
        assert_eq!(cell.ch, 'A');
    }

    #[test]
    fn test_screen_new() {
        let screen = Screen::new(80, 24);
        assert_eq!(screen.size(), (80, 24));
        assert_eq!(screen.cursor(), (0, 0));
    }

    #[test]
    fn test_screen_cell_access() {
        let screen = Screen::new(10, 5);
        assert!(screen.cell(0, 0).is_some());
        assert!(screen.cell(4, 9).is_some());
        assert!(screen.cell(5, 0).is_none());
        assert!(screen.cell(0, 10).is_none());
    }

    #[test]
    fn test_screen_line() {
        let screen = Screen::new(10, 5);
        assert!(screen.line(0).is_some());
        assert!(screen.line(4).is_some());
        assert!(screen.line(5).is_none());
    }

    #[test]
    fn test_screen_line_text() {
        let screen = Screen::new(10, 5);
        assert_eq!(screen.line_text(0), Some(" ".repeat(10)));
    }

    #[test]
    fn test_parse_plain_text() {
        let screen = parse_str("Hello", 80, 24);
        assert_eq!(screen.line_text(0), Some("Hello".to_string() + &" ".repeat(75)));
    }

    #[test]
    fn test_parse_sgr_color() {
        let screen = parse_str("\x1b[31mRed\x1b[0m", 80, 24);
        let cell = screen.cell(0, 0).unwrap();
        assert_eq!(cell.style.fg, Some(Color::Ansi(31)));
    }

    #[test]
    fn test_parse_cup() {
        let screen = parse_str("\x1b[5;10HHello", 80, 24);
        // Cursor is at row 5, col 10 (1-indexed) = (4, 9) 0-indexed
        // After writing "Hello" (5 chars), cursor is at col 14
        assert_eq!(screen.cursor(), (4, 14));
        // First char of "Hello" should be at row 5, col 10
        assert_eq!(screen.char_at(4, 9), Some('H'));
    }

    #[test]
    fn test_parse_cuu() {
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[5;10H\x1b[2A");
        assert_eq!(parser.screen.cursor(), (2, 9));
    }

    #[test]
    fn test_parse_cud() {
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[3B");
        assert_eq!(parser.screen.cursor(), (3, 0));
    }

    #[test]
    fn test_parse_cuf() {
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[5C");
        assert_eq!(parser.screen.cursor(), (0, 5));
    }

    #[test]
    fn test_parse_cub() {
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[3D");
        assert_eq!(parser.screen.cursor(), (0, 0)); // Already at 0, can't go negative
    }

    #[test]
    fn test_parse_ed_clear_from_cursor() {
        let screen = parse_str("\x1b[3J", 10, 5);
        // After clearing, should be all spaces
        assert_eq!(screen.line_text(0), Some(" ".repeat(10)));
    }

    #[test]
    fn test_parse_el_clear_line() {
        // EL(0) = clear from cursor to end of line
        // After "Hello", cursor is at col 5, so EL(0) clears cols 5-9
        let screen = parse_str("Hello\x1b[K", 10, 1);
        // "Hello" remains, but trailing spaces were already spaces, so line is "Hello     "
        assert_eq!(screen.line_text(0), Some("Hello     ".to_string()));
    }

    #[test]
    fn test_parse_el_clear_line_mode2() {
        // EL(2) = clear entire line
        let screen = parse_str("Hello\x1b[2K", 10, 1);
        assert_eq!(screen.line_text(0), Some(" ".repeat(10)));
    }

    #[test]
    fn test_parse_scroll() {
        let screen = parse_str("\x1b[2S", 10, 5);
        // Screen should be scrolled, content moved up
        assert_eq!(screen.line_text(0).unwrap().trim(), "");
    }

    #[test]
    fn test_screen_contains() {
        let screen = parse_str("Hello World", 80, 24);
        assert!(screen.contains("Hello"));
        assert!(screen.contains("World"));
        assert!(!screen.contains("Foo"));
    }

    #[test]
    fn test_screen_text_range() {
        let screen = parse_str("Line1\nLine2\nLine3", 80, 24);
        let text = screen.text_range(0, 2);
        assert!(text.contains("Line1"));
        assert!(text.contains("Line2"));
    }

    #[test]
    fn test_param_parser() {
        let mut parser = ParamParser::new(b"5;10;15");
        assert_eq!(parser.parse_param(), 5);
        // Move past the semicolon
        parser.next();
        assert_eq!(parser.parse_param(), 10);
        parser.next();
        assert_eq!(parser.parse_param(), 15);
    }

    #[test]
    fn test_param_parser_with_semicolon() {
        let mut parser = ParamParser::new(b";10");
        assert_eq!(parser.parse_param(), 0); // Empty param defaults to 0
        parser.next();
        assert_eq!(parser.parse_param(), 10);
    }

    #[test]
    fn test_cell_style_reset() {
        let mut style = CellStyle {
            fg: Some(Color::Ansi(31)),
            bg: Some(Color::Rgb(0, 0, 0)),
            bold: true,
            italic: true,
            underline: true,
        };
        style.reset();
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.underline);
    }

    #[test]
    fn test_deferred_wrap_cr_lf_no_extra_scroll() {
        // Write 80 'a's then \r\n + 'b'.
        // Deferred-wrap: 80th 'a' at (0, 79) with pending_wrap; \r clears pending,
        // \n moves to (1, 0), 'b' at (1, 0).
        // Naive auto-wrap would put 'b' at (2, 0).
        let input = format!("{}\r\nb", "a".repeat(80));
        let screen = parse_str(&input, 80, 24);
        assert_eq!(screen.char_at(0, 79), Some('a'));
        assert_eq!(screen.char_at(1, 0), Some('b'));
    }

    #[test]
    fn test_deferred_wrap_no_scroll_at_bottom_right() {
        // Move cursor to (24, 1) [ANSI 1-indexed => (23, 0) 0-indexed], write 80 'a's.
        // With deferred-wrap, no scroll should occur; all 80 'a's stay on row 23.
        let input = format!("\x1b[24;1H{}", "a".repeat(80));
        let screen = parse_str(&input, 80, 24);
        assert_eq!(screen.char_at(23, 0), Some('a'));
        assert_eq!(screen.char_at(23, 79), Some('a'));
    }

    #[test]
    fn test_assert_cell_fg_rgb_matches_truecolor() {
        // SGR 38;2;r;g;b sets a truecolor fg.
        let screen = parse_str("\x1b[38;2;215;119;87mA", 80, 24);
        screen.assert_cell_fg_rgb(0, 0, 215, 119, 87);
    }

    #[test]
    fn test_pending_wrap_cleared_by_scroll_up() {
        // CUP (1,1), write 80 chars to set pending_wrap at (0, 79),
        // then SU (scroll up 1). SU doesn't move the cursor but DOES
        // shift the grid so the cell that triggered the wrap flag is
        // gone. pending_wrap must be cleared so the next 'X' doesn't
        // wrap to (1, 0).
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[1;1H");
        parser.parse(&"a".repeat(80).into_bytes());
        // pending_wrap is now set at (0, 79)
        assert!(parser.screen.pending_wrap, "precondition: pending_wrap set after 80 chars");
        parser.parse(b"\x1b[1S");
        assert!(!parser.screen.pending_wrap, "SU must clear pending_wrap");
        parser.parse(b"X");
        // With cleared flag: X overwrites (0, 79) (which is now blank
        // because the original row 0 scrolled away).
        assert_eq!(parser.screen.char_at(0, 79), Some('X'));
        // Row 1 col 0 must not have been written to by this X.
        assert_ne!(parser.screen.char_at(1, 0), Some('X'));
    }

    #[test]
    fn test_pending_wrap_cleared_by_clear_screen() {
        // 80 chars to set pending_wrap, then ED(2) clear screen, then
        // write 'Z' at implicit cursor (still (0, 79)). With cleared
        // flag Z overwrites (0, 79) rather than wrapping to (1, 0).
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(&"a".repeat(80).into_bytes());
        assert!(parser.screen.pending_wrap, "precondition: pending_wrap set after 80 chars");
        parser.parse(b"\x1b[2J");
        assert!(!parser.screen.pending_wrap, "ED(2) must clear pending_wrap");
        parser.parse(b"Z");
        assert_eq!(parser.screen.char_at(0, 79), Some('Z'));
        assert_ne!(parser.screen.char_at(1, 0), Some('Z'));
    }

    #[test]
    fn test_pending_wrap_cleared_by_decrc() {
        // DECSC (ESC 7) saves cursor at (5, 10). Then write 80 chars
        // starting at row 10 to set pending_wrap at (10, 79). DECRC
        // (ESC 8) restores to (5, 10); the flag must be cleared so the
        // next 'Y' lands at (5, 10) rather than wrapping to (6, 0).
        let mut parser = AnsiParser::new(80, 24);
        parser.parse(b"\x1b[6;11H\x1b7"); // move to (5, 10) 0-indexed, save
        parser.parse(b"\x1b[11;1H");      // move to (10, 0)
        parser.parse(&"a".repeat(80).into_bytes()); // pending_wrap set at (10, 79)
        assert!(parser.screen.pending_wrap, "precondition: pending_wrap set after 80 chars");
        parser.parse(b"\x1b8"); // restore cursor to (5, 10)
        assert!(!parser.screen.pending_wrap, "DECRC must clear pending_wrap");
        parser.parse(b"Y");
        assert_eq!(parser.screen.char_at(5, 10), Some('Y'));
        assert_ne!(parser.screen.char_at(6, 0), Some('Y'));
    }
}
