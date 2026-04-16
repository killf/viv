use std::io::Write as IoWrite;
use crate::terminal::size::{TermSize, terminal_size};
use crate::terminal::raw_mode::RawMode;

pub trait Backend {
    fn size(&self) -> crate::Result<TermSize>;
    fn write(&mut self, buf: &[u8]) -> crate::Result<()>;
    fn flush(&mut self) -> crate::Result<()>;
    fn enable_raw_mode(&mut self) -> crate::Result<()>;
    fn disable_raw_mode(&mut self) -> crate::Result<()>;
    fn hide_cursor(&mut self) -> crate::Result<()>;
    fn show_cursor(&mut self) -> crate::Result<()>;
    fn move_cursor(&mut self, row: u16, col: u16) -> crate::Result<()>;
}

// ── LinuxBackend ──────────────────────────────────────────────────────────────

pub struct LinuxBackend {
    stdout: std::io::Stdout,
    raw_mode: Option<RawMode>,
}

impl LinuxBackend {
    pub fn new() -> Self {
        LinuxBackend {
            stdout: std::io::stdout(),
            raw_mode: None,
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxBackend {
    fn drop(&mut self) {
        // Dropping raw_mode restores the original terminal settings via RawMode::drop.
        self.raw_mode = None;
    }
}

impl Backend for LinuxBackend {
    fn size(&self) -> crate::Result<TermSize> {
        terminal_size()
    }

    fn write(&mut self, buf: &[u8]) -> crate::Result<()> {
        self.stdout.write_all(buf)?;
        Ok(())
    }

    fn flush(&mut self) -> crate::Result<()> {
        self.stdout.flush()?;
        Ok(())
    }

    fn enable_raw_mode(&mut self) -> crate::Result<()> {
        if self.raw_mode.is_none() {
            self.raw_mode = Some(RawMode::enable(0)?);
        }
        Ok(())
    }

    fn disable_raw_mode(&mut self) -> crate::Result<()> {
        self.raw_mode = None;
        Ok(())
    }

    fn hide_cursor(&mut self) -> crate::Result<()> {
        self.stdout.write_all(b"\x1b[?25l")?;
        Ok(())
    }

    fn show_cursor(&mut self) -> crate::Result<()> {
        self.stdout.write_all(b"\x1b[?25h")?;
        Ok(())
    }

    fn move_cursor(&mut self, row: u16, col: u16) -> crate::Result<()> {
        let seq = format!("\x1b[{};{}H", row + 1, col + 1);
        self.stdout.write_all(seq.as_bytes())?;
        Ok(())
    }
}

// ── TestBackend ───────────────────────────────────────────────────────────────

pub struct TestBackend {
    pub output: Vec<u8>,
    pub size: TermSize,
    pub raw_mode_enabled: bool,
    pub cursor_visible: bool,
    pub cursor_pos: (u16, u16),
}

impl TestBackend {
    pub fn new(cols: u16, rows: u16) -> Self {
        TestBackend {
            output: Vec::new(),
            size: TermSize { cols, rows },
            raw_mode_enabled: false,
            cursor_visible: true,
            cursor_pos: (0, 0),
        }
    }
}

impl Backend for TestBackend {
    fn size(&self) -> crate::Result<TermSize> {
        Ok(self.size)
    }

    fn write(&mut self, buf: &[u8]) -> crate::Result<()> {
        self.output.extend_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> crate::Result<()> {
        Ok(())
    }

    fn enable_raw_mode(&mut self) -> crate::Result<()> {
        self.raw_mode_enabled = true;
        Ok(())
    }

    fn disable_raw_mode(&mut self) -> crate::Result<()> {
        self.raw_mode_enabled = false;
        Ok(())
    }

    fn hide_cursor(&mut self) -> crate::Result<()> {
        self.cursor_visible = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> crate::Result<()> {
        self.cursor_visible = true;
        Ok(())
    }

    fn move_cursor(&mut self, row: u16, col: u16) -> crate::Result<()> {
        self.cursor_pos = (row, col);
        Ok(())
    }
}
