use crate::core::terminal::backend::Backend;
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::size::TermSize;

/// Double-buffered renderer: widgets paint into `current`, then `flush` diffs
/// it against `previous`, writes only changed cells to the backend, and swaps.
pub struct Renderer {
    current: Buffer,
    previous: Buffer,
    size: TermSize,
    last_cursor: Option<(u16, u16)>,
}

impl Renderer {
    /// Create two empty buffers covering the full terminal area.
    pub fn new(size: TermSize) -> Self {
        let area = Rect::new(0, 0, size.cols, size.rows);
        Renderer {
            current: Buffer::empty(area),
            previous: Buffer::empty(area),
            size,
            last_cursor: None,
        }
    }

    /// Recreate both buffers at the new terminal size.
    pub fn resize(&mut self, size: TermSize) {
        let area = Rect::new(0, 0, size.cols, size.rows);
        self.current = Buffer::empty(area);
        self.previous = Buffer::empty(area);
        self.size = size;
        self.last_cursor = None;
    }

    /// Returns the current buffer for widgets to render into.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.current
    }

    /// Returns a Rect covering the full terminal area.
    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.size.cols, self.size.rows)
    }

    /// Flush the current frame to the backend using a diff against the previous frame.
    ///
    /// When there is a diff, we wrap it in a DEC synchronized update
    /// (`\x1b[?2026h/l`) and move the cursor to `cursor` inside the same block,
    /// so the terminal commits cells + final cursor position atomically. We do
    /// *not* emit `hide_cursor` / `show_cursor` — sending `\x1b[?25h` every
    /// frame resets the terminal's blink phase, which made the caret appear to
    /// blink at irregular rates during streaming or spinner animation.
    ///
    /// When the buffer is unchanged but `cursor` moved (e.g. the user pressed
    /// an arrow key inside the input), we emit just the move.
    ///
    /// Pass `cursor: None` to leave the cursor alone (used by tests).
    pub fn flush(
        &mut self,
        backend: &mut dyn Backend,
        cursor: Option<(u16, u16)>,
    ) -> crate::Result<()> {
        let diff = self.current.diff(&self.previous);

        if !diff.is_empty() {
            backend.write(b"\x1b[?2026h")?;
            backend.write(&diff)?;
            if let Some((col, row)) = cursor {
                backend.move_cursor(row, col)?;
                self.last_cursor = Some((col, row));
            }
            backend.write(b"\x1b[?2026l")?;
            backend.flush()?;
        } else if let Some((col, row)) = cursor
            && self.last_cursor != Some((col, row))
        {
            backend.move_cursor(row, col)?;
            backend.flush()?;
            self.last_cursor = Some((col, row));
        }

        // Swap buffers regardless so the buffer bookkeeping stays consistent.
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
        Ok(())
    }
}
