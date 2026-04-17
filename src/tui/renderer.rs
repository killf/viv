use crate::core::terminal::backend::Backend;
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::size::TermSize;

/// Double-buffered renderer: widgets paint into `current`, then `flush` diffs
/// it against `previous`, writes only changed cells to the backend, and swaps.
pub struct Renderer {
    current: Buffer,
    previous: Buffer,
    size: TermSize,
}

impl Renderer {
    /// Create two empty buffers covering the full terminal area.
    pub fn new(size: TermSize) -> Self {
        let area = Rect::new(0, 0, size.cols, size.rows);
        Renderer {
            current: Buffer::empty(area),
            previous: Buffer::empty(area),
            size,
        }
    }

    /// Recreate both buffers at the new terminal size.
    pub fn resize(&mut self, size: TermSize) {
        let area = Rect::new(0, 0, size.cols, size.rows);
        self.current = Buffer::empty(area);
        self.previous = Buffer::empty(area);
        self.size = size;
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
    /// If the diff is empty, this is a complete no-op. When there *is* a diff, the
    /// cursor is hidden for the duration of the redraw and stays hidden on return —
    /// writing the diff leaves the physical cursor at an arbitrary cell, so the
    /// caller must move it to the desired position and call `show_cursor()` before
    /// the next frame.
    pub fn flush(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        let diff = self.current.diff(&self.previous);

        if !diff.is_empty() {
            backend.write(b"\x1b[?2026h")?;
            backend.hide_cursor()?;
            backend.write(&diff)?;
            backend.write(b"\x1b[?2026l")?;
            backend.flush()?;
        }

        // Swap buffers regardless so the buffer bookkeeping stays consistent.
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
        Ok(())
    }
}
