/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

#[cfg(unix)]
mod unix_impl {
    use super::TermSize;

    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    // TIOCGWINSZ ioctl number on Linux x86_64
    const TIOCGWINSZ: u64 = 0x5413;

    unsafe extern "C" {
        fn ioctl(fd: i32, request: u64, ...) -> i32;
    }

    /// Query the terminal size from stdout (fd 1).
    /// Falls back to 80x24 on ioctl failure or zero dimensions.
    pub fn terminal_size() -> crate::Result<TermSize> {
        let mut ws = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
        let ret = unsafe { ioctl(1, TIOCGWINSZ, &mut ws) };
        if ret == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            Ok(TermSize { cols: ws.ws_col, rows: ws.ws_row })
        } else {
            Ok(TermSize { cols: 80, rows: 24 })
        }
    }
}

#[cfg(unix)]
pub use unix_impl::terminal_size;
