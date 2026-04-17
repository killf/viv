use viv::tui::renderer::*;
use viv::terminal::backend::TestBackend;
use viv::terminal::size::TermSize;
use viv::terminal::buffer::Rect;

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
    r.flush(&mut backend).unwrap();
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
    r.flush(&mut backend).unwrap();
    // Cursor should still be visible — we didn't hide/show it since nothing changed.
    assert!(backend.cursor_visible);
}

#[test]
fn flush_after_change_writes_diff() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'X');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend).unwrap();
    let out = String::from_utf8_lossy(&backend.output);
    assert!(out.contains('X'));
}

#[test]
fn second_flush_same_content_no_diff() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'A');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend).unwrap();
    let first_len = backend.output.len();

    // Second flush: repaint the same content (widgets must repaint each frame)
    backend.output.clear();
    r.buffer_mut().set_char(0, 0, 'A');
    r.flush(&mut backend).unwrap();
    // Should be just sync sequences, no cell diff (same content as previous)
    assert!(backend.output.len() < first_len);
}

#[test]
fn resize_updates_area() {
    let mut r = Renderer::new(TermSize { cols: 80, rows: 24 });
    r.resize(TermSize { cols: 120, rows: 40 });
    assert_eq!(r.area(), Rect::new(0, 0, 120, 40));
}
