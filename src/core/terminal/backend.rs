use super::size::TermSize;
use crate::core::platform::PlatformTerminal;
use std::io::Write as IoWrite;

/// ANSI sequence to switch to the terminal's alternate screen buffer
/// (like vim/htop). Preserves the user's scrollback.
pub const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h";

/// ANSI sequence to leave the alternate screen buffer and restore the prior view.
pub const LEAVE_ALT_SCREEN: &[u8] = b"\x1b[?1049l";

/// ANSI sequence to enable basic mouse mode (1000) — reports button press/release.
pub const ENABLE_MOUSE_1000: &[u8] = b"\x1b[?1000h";

/// ANSI sequence to enable SGR mouse mode (1006) — reports wheel events.
pub const ENABLE_SGR_MOUSE: &[u8] = b"\x1b[?1006h";

/// ANSI sequence to enable URXVT mouse mode (1015) — fallback for terminals without SGR.
pub const ENABLE_URXVT_MOUSE: &[u8] = b"\x1b[?1015h";

/// ANSI sequence to disable basic mouse mode (1000).
pub const DISABLE_MOUSE_1000: &[u8] = b"\x1b[?1000l";

/// ANSI sequence to disable SGR mouse mode.
pub const DISABLE_SGR_MOUSE: &[u8] = b"\x1b[?1006l";

/// ANSI sequence to disable URXVT mouse mode.
pub const DISABLE_URXVT_MOUSE: &[u8] = b"\x1b[?1015l";

pub trait Backend {
    fn size(&self) -> crate::Result<TermSize>;
    fn write(&mut self, buf: &[u8]) -> crate::Result<()>;
    fn flush(&mut self) -> crate::Result<()>;
    fn enable_raw_mode(&mut self) -> crate::Result<()>;
    fn disable_raw_mode(&mut self) -> crate::Result<()>;
    fn hide_cursor(&mut self) -> crate::Result<()>;
    fn show_cursor(&mut self) -> crate::Result<()>;
    fn move_cursor(&mut self, row: u16, col: u16) -> crate::Result<()>;
    /// Switch to the terminal's alternate screen buffer.
    fn enter_alt_screen(&mut self) -> crate::Result<()>;
    /// Leave the alternate screen buffer, restoring the prior view.
    fn leave_alt_screen(&mut self) -> crate::Result<()>;
}

// ── LinuxBackend ──────────────────────────────────────────────────────────────

#[cfg(unix)]
use super::raw_mode::RawMode;
#[cfg(unix)]
use super::size::terminal_size;

#[cfg(unix)]
pub struct LinuxBackend {
    stdout: std::io::Stdout,
    raw_mode: Option<RawMode>,
    in_alt_screen: bool,
    mouse_enabled: bool,
}

#[cfg(unix)]
impl LinuxBackend {
    pub fn new() -> Self {
        LinuxBackend {
            stdout: std::io::stdout(),
            raw_mode: None,
            in_alt_screen: false,
            mouse_enabled: false,
        }
    }
}

#[cfg(unix)]
impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
impl Drop for LinuxBackend {
    fn drop(&mut self) {
        // Always restore the main screen before dropping raw mode, so the
        // user's scrollback is preserved even on panic / early return.
        if self.in_alt_screen {
            let _ = self.stdout.write_all(LEAVE_ALT_SCREEN);
            let _ = self.stdout.flush();
        }
        // Disable mouse mode if enabled
        if self.mouse_enabled {
            let _ = self.stdout.write_all(DISABLE_MOUSE_1000);
            let _ = self.stdout.write_all(DISABLE_SGR_MOUSE);
            let _ = self.stdout.write_all(DISABLE_URXVT_MOUSE);
            let _ = self.stdout.flush();
        }
        // Dropping raw_mode restores the original terminal settings via RawMode::drop.
        self.raw_mode = None;
    }
}

#[cfg(unix)]
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

    fn enter_alt_screen(&mut self) -> crate::Result<()> {
        if !self.in_alt_screen {
            self.stdout.write_all(ENTER_ALT_SCREEN)?;
            self.stdout.write_all(ENABLE_SGR_MOUSE)?;
            self.stdout.write_all(ENABLE_URXVT_MOUSE)?;
            self.stdout.flush()?;
            self.in_alt_screen = true;
            self.mouse_enabled = true;
        }
        Ok(())
    }

    fn leave_alt_screen(&mut self) -> crate::Result<()> {
        if self.in_alt_screen {
            if self.mouse_enabled {
                self.stdout.write_all(DISABLE_MOUSE_1000)?;
                self.stdout.write_all(DISABLE_SGR_MOUSE)?;
                self.stdout.write_all(DISABLE_URXVT_MOUSE)?;
                self.mouse_enabled = false;
            }
            self.stdout.write_all(LEAVE_ALT_SCREEN)?;
            self.stdout.flush()?;
            self.in_alt_screen = false;
        }
        Ok(())
    }
}

// ── CrossBackend ─────────────────────────────────────────────────────────────

pub struct CrossBackend {
    terminal: PlatformTerminal,
    stdout: std::io::Stdout,
    in_alt_screen: bool,
    mouse_enabled: bool,
}

impl CrossBackend {
    pub fn new() -> crate::Result<Self> {
        Ok(CrossBackend {
            terminal: PlatformTerminal::new()?,
            stdout: std::io::stdout(),
            in_alt_screen: false,
            mouse_enabled: false,
        })
    }
}

impl Drop for CrossBackend {
    fn drop(&mut self) {
        if self.mouse_enabled {
            let _ = self.stdout.write_all(DISABLE_MOUSE_1000);
            let _ = self.stdout.write_all(DISABLE_SGR_MOUSE);
            let _ = self.stdout.write_all(DISABLE_URXVT_MOUSE);
            let _ = self.stdout.flush();
        }
        if self.in_alt_screen {
            let _ = self.stdout.write_all(LEAVE_ALT_SCREEN);
            let _ = self.stdout.flush();
        }
        self.terminal.disable_raw_mode().ok();
    }
}

impl Backend for CrossBackend {
    fn size(&self) -> crate::Result<TermSize> {
        let (rows, cols) = self.terminal.size()?;
        Ok(TermSize { rows, cols })
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
        self.terminal.enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> crate::Result<()> {
        self.terminal.disable_raw_mode()
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

    fn enter_alt_screen(&mut self) -> crate::Result<()> {
        if !self.in_alt_screen {
            self.stdout.write_all(ENTER_ALT_SCREEN)?;
            self.stdout.write_all(ENABLE_SGR_MOUSE)?;
            self.stdout.write_all(ENABLE_URXVT_MOUSE)?;
            self.stdout.flush()?;
            self.in_alt_screen = true;
            self.mouse_enabled = true;
        }
        Ok(())
    }

    fn leave_alt_screen(&mut self) -> crate::Result<()> {
        if self.in_alt_screen {
            if self.mouse_enabled {
                self.stdout.write_all(DISABLE_MOUSE_1000)?;
                self.stdout.write_all(DISABLE_SGR_MOUSE)?;
                self.stdout.write_all(DISABLE_URXVT_MOUSE)?;
                self.mouse_enabled = false;
            }
            self.stdout.write_all(LEAVE_ALT_SCREEN)?;
            self.stdout.flush()?;
            self.in_alt_screen = false;
        }
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
    pub in_alt_screen: bool,
}

impl TestBackend {
    pub fn new(cols: u16, rows: u16) -> Self {
        TestBackend {
            output: Vec::new(),
            size: TermSize { cols, rows },
            raw_mode_enabled: false,
            cursor_visible: true,
            cursor_pos: (0, 0),
            in_alt_screen: false,
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

    fn enter_alt_screen(&mut self) -> crate::Result<()> {
        self.in_alt_screen = true;
        Ok(())
    }

    fn leave_alt_screen(&mut self) -> crate::Result<()> {
        self.in_alt_screen = false;
        Ok(())
    }
}
