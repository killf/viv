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
    /// If the diff is empty (no cells changed), this is a complete no-op: we don't
    /// touch the cursor or emit any bytes. This avoids the hardware cursor flickering
    /// at the event-loop's frame rate.
    pub fn flush(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        let diff = self.current.diff(&self.previous);

        if !diff.is_empty() {
            // 1. Begin synchronized update
            backend.write(b"\x1b[?2026h")?;
            // 2. Hide cursor during redraw
            backend.hide_cursor()?;
            // 3. Write diff bytes
            backend.write(&diff)?;
            // 4. Show cursor again
            backend.show_cursor()?;
            // 5. End synchronized update
            backend.write(b"\x1b[?2026l")?;
            // 6. Flush backend
            backend.flush()?;
        }

        // Swap buffers regardless so the buffer bookkeeping stays consistent.
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
        Ok(())
    }
}
