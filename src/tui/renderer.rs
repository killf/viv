use crate::terminal::backend::Backend;
use crate::terminal::buffer::{Buffer, Rect};
use crate::terminal::size::TermSize;

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
    /// Steps:
    /// 1. Begin synchronized update (`\x1b[?2026h`)
    /// 2. Hide cursor
    /// 3. Compute diff between current and previous buffers
    /// 4. Write diff bytes to backend
    /// 5. Show cursor
    /// 6. End synchronized update (`\x1b[?2026l`)
    /// 7. Flush backend
    /// 8. Swap current ↔ previous
    /// 9. Clear the new current buffer (ready for next frame)
    pub fn flush(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        // 1. Begin synchronized update
        backend.write(b"\x1b[?2026h")?;
        // 2. Hide cursor
        backend.hide_cursor()?;
        // 3. Compute diff
        let diff = self.current.diff(&self.previous);
        // 4. Write diff
        backend.write(&diff)?;
        // 5. Show cursor
        backend.show_cursor()?;
        // 6. End synchronized update
        backend.write(b"\x1b[?2026l")?;
        // 7. Flush backend
        backend.flush()?;
        // 8. Swap buffers
        std::mem::swap(&mut self.current, &mut self.previous);
        // 9. Reset the new current buffer to match the previous (displayed) state
        //    so widgets only need to paint what changes, and diff is minimal.
        self.current.clone_from(&self.previous);
        Ok(())
    }
}
