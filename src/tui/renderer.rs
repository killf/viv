use crate::core::terminal::backend::Backend;
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::size::TermSize;
use crate::tui::text_map::TextMap;
use std::cell::{RefCell, RefMut};

/// Double-buffered renderer: widgets paint into `current`, then `flush` diffs
/// it against `previous`, writes only changed cells to the backend, and swaps.
pub struct Renderer {
    current: RefCell<Buffer>,
    previous: RefCell<Buffer>,
    size: TermSize,
    last_cursor: Option<(u16, u16)>,
    /// Current selection region for highlighting (inverted fg/bg colors).
    pub(crate) selection: Option<Rect>,
    /// Maps screen coordinates to text sources for Ctrl+C copy.
    text_map: RefCell<TextMap>,
}

impl Renderer {
    /// Create two empty buffers covering the full terminal area.
    pub fn new(size: TermSize) -> Self {
        let area = Rect::new(0, 0, size.cols, size.rows);
        Renderer {
            current: RefCell::new(Buffer::empty(area)),
            previous: RefCell::new(Buffer::empty(area)),
            size,
            last_cursor: None,
            selection: None,
            text_map: RefCell::new(TextMap::new()),
        }
    }

    /// Recreate both buffers at the new terminal size.
    pub fn resize(&mut self, size: TermSize) {
        let area = Rect::new(0, 0, size.cols, size.rows);
        self.current.replace(Buffer::empty(area));
        self.previous.replace(Buffer::empty(area));
        self.size = size;
        self.last_cursor = None;
        self.selection = None;
        self.text_map.borrow_mut().clear();
    }

    /// Set the selection region for highlighting.
    pub fn set_selection(&mut self, rect: Option<Rect>) {
        self.selection = rect;
    }

    /// Clear any active selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Returns a mutable reference to the current buffer for widgets to render into.
    /// Takes `&self` (not `&mut self`) because Buffer is wrapped in RefCell for interior mutability.
    pub fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.current.borrow_mut()
    }

    /// Returns a reference to the text map (for text extraction / Ctrl+C copy).
    pub fn text_map(&self) -> std::cell::Ref<'_, TextMap> {
        self.text_map.borrow()
    }

    /// Returns a mutable reference to the text map for building mappings during render.
    /// Takes `&self` (not `&mut self`) because TextMap is wrapped in RefCell for interior mutability.
    pub fn text_map_mut(&self) -> RefMut<'_, TextMap> {
        self.text_map.borrow_mut()
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
    ///
    /// Selection highlighting: before computing the diff, we swap fg/bg colors
    /// for cells in the selection region so they appear inverted. This way the
    /// diff includes those cells (since previous frame had no inversion).
    pub fn flush(
        &mut self,
        backend: &mut dyn Backend,
        cursor: Option<(u16, u16)>,
    ) -> crate::Result<()> {
        // Apply selection highlighting to current buffer cells in the selection region.
        // This makes the current frame's selection cells differ from the previous
        // frame, so they will be included in the diff and written to the terminal.
        {
            let mut current = self.current.borrow_mut();
            if let Some(sel) = &self.selection {
                for row in sel.y..sel.y.saturating_add(sel.height) {
                    for col in sel.x..sel.x.saturating_add(sel.width) {
                        let cell = current.get_mut(col, row);
                        std::mem::swap(&mut cell.fg, &mut cell.bg);
                    }
                }
            }
        }

        let diff = {
            let current = self.current.borrow();
            let previous = self.previous.borrow();
            current.diff(&previous)
        };

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
        // Use mem::swap on the dereferenced RefMuts.
        let mut current = self.current.borrow_mut();
        let mut previous = self.previous.borrow_mut();
        std::mem::swap(&mut *current, &mut *previous);
        current.clear();
        Ok(())
    }
}
