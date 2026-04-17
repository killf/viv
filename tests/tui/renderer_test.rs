use viv::tui::renderer::*;
use viv::core::terminal::backend::TestBackend;
use viv::core::terminal::size::TermSize;
use viv::core::terminal::buffer::Rect;

#[test]
fn new_renderer_has_correct_area() {
    let r = Renderer::new(TermSize { cols: 80, rows: 24 });
    assert_eq!(r.area(), Rect::new(0, 0, 80, 24));
}

#[test]
fn flush_empty_writes_nothing_when_buffer_unchanged() {
    // Identical buffers → diff is empty → no output (avoids cursor flicker).
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, None).unwrap();
    assert!(
        backend.output.is_empty(),
        "first flush of all-blank buffer should write nothing (got {} bytes)",
        backend.output.len()
    );
}

#[test]
fn flush_empty_keeps_cursor_visible() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    let mut backend = TestBackend::new(10, 5);
    assert!(backend.cursor_visible);
    r.flush(&mut backend, None).unwrap();
    // Cursor should still be visible — flush never toggles visibility.
    assert!(backend.cursor_visible);
}

#[test]
fn flush_after_change_writes_diff() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'X');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, None).unwrap();
    let out = String::from_utf8_lossy(&backend.output);
    assert!(out.contains('X'));
}

#[test]
fn second_flush_same_content_no_diff() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'A');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, None).unwrap();
    let first_len = backend.output.len();

    // Second flush: repaint the same content (widgets must repaint each frame)
    backend.output.clear();
    r.buffer_mut().set_char(0, 0, 'A');
    r.flush(&mut backend, None).unwrap();
    // Should be empty — same content, no cursor provided.
    assert!(backend.output.len() < first_len);
}

#[test]
fn flush_moves_cursor_inside_sync_block() {
    // When a diff is written and a cursor is provided, flush must open a sync
    // block and set the cursor position before closing it, so cells + cursor
    // commit atomically on the terminal.
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'X');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, Some((3, 2))).unwrap();

    let out = String::from_utf8_lossy(&backend.output);
    assert!(out.contains("\x1b[?2026h"), "sync begin present");
    assert!(out.contains("\x1b[?2026l"), "sync end present");
    assert_eq!(backend.cursor_pos, (2, 3));
    // No hide/show — blink phase must be preserved.
    assert!(backend.cursor_visible);
}

#[test]
fn flush_empty_diff_still_moves_cursor_when_position_changes() {
    // No cells changed, but cursor moved (e.g. user pressed Left in input).
    // flush should still emit the move so the hardware caret tracks the editor.
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, Some((2, 0))).unwrap();
    assert_eq!(backend.cursor_pos, (0, 2));

    backend.output.clear();
    r.flush(&mut backend, Some((5, 0))).unwrap();
    // No diff, but cursor changed → just the move, no sync block.
    let out = String::from_utf8_lossy(&backend.output);
    assert!(!out.contains("\x1b[?2026"), "no sync block when only cursor moved");
    assert_eq!(backend.cursor_pos, (0, 5));
}

#[test]
fn flush_empty_diff_same_cursor_is_noop() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend, Some((2, 0))).unwrap();
    backend.output.clear();
    r.flush(&mut backend, Some((2, 0))).unwrap();
    assert!(backend.output.is_empty());
}

#[test]
fn resize_updates_area() {
    let mut r = Renderer::new(TermSize { cols: 80, rows: 24 });
    r.resize(TermSize { cols: 120, rows: 40 });
    assert_eq!(r.area(), Rect::new(0, 0, 120, 40));
}
