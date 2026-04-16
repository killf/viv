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
fn flush_empty_writes_sync_sequences() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend).unwrap();
    let out = String::from_utf8_lossy(&backend.output);
    // Should contain sync begin/end and cursor hide/show, but no cell diffs
    assert!(out.contains("\x1b[?2026h")); // sync begin
    assert!(out.contains("\x1b[?2026l")); // sync end
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
fn second_flush_no_change_no_diff() {
    let mut r = Renderer::new(TermSize { cols: 10, rows: 5 });
    r.buffer_mut().set_char(0, 0, 'A');
    let mut backend = TestBackend::new(10, 5);
    r.flush(&mut backend).unwrap();
    let first_len = backend.output.len();

    // Second flush with no changes
    backend.output.clear();
    r.flush(&mut backend).unwrap();
    // Should be just sync sequences, no cell diff
    assert!(backend.output.len() < first_len);
}

#[test]
fn resize_updates_area() {
    let mut r = Renderer::new(TermSize { cols: 80, rows: 24 });
    r.resize(TermSize { cols: 120, rows: 40 });
    assert_eq!(r.area(), Rect::new(0, 0, 120, 40));
}
