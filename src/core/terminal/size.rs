use crate::core::platform;

/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

pub fn terminal_size() -> crate::Result<TermSize> {
    let (rows, cols) = platform::terminal_size()?;
    Ok(TermSize { rows, cols })
}
