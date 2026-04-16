use viv::terminal::screen::{Cell, Screen};

// --- Cell default ---

#[test]
fn cell_default_is_blank() {
    let c = Cell::default();
    assert_eq!(c.ch, ' ');
    assert_eq!(c.fg, None);
    assert!(!c.bold);
}

// --- Screen::new produces blank screens ---

#[test]
fn new_screen_all_default_cells() {
    let s = Screen::new(4, 3);
    for row in 0..3 {
        for col in 0..4 {
            assert_eq!(s.get(row, col), Cell::default(), "row={row} col={col}");
        }
    }
}

// --- put / get round-trip ---

#[test]
fn put_then_get_char() {
    let mut s = Screen::new(10, 5);
    s.put(2, 3, 'X');
    let cell = s.get(2, 3);
    assert_eq!(cell.ch, 'X');
    assert_eq!(cell.fg, None);
    assert!(!cell.bold);
}

#[test]
fn put_styled_then_get() {
    let mut s = Screen::new(10, 5);
    s.put_styled(1, 1, 'Z', Some(32), true);
    let cell = s.get(1, 1);
    assert_eq!(cell.ch, 'Z');
    assert_eq!(cell.fg, Some(32));
    assert!(cell.bold);
}

// --- put_str ---

#[test]
fn put_str_writes_chars_left_to_right() {
    let mut s = Screen::new(20, 5);
    s.put_str(0, 0, "hello");
    assert_eq!(s.get(0, 0).ch, 'h');
    assert_eq!(s.get(0, 1).ch, 'e');
    assert_eq!(s.get(0, 2).ch, 'l');
    assert_eq!(s.get(0, 3).ch, 'l');
    assert_eq!(s.get(0, 4).ch, 'o');
    // char after string is untouched
    assert_eq!(s.get(0, 5).ch, ' ');
}

// --- diff: two blank screens ---

#[test]
fn diff_two_empty_screens_returns_empty() {
    let mut s = Screen::new(4, 3);
    let bytes = s.diff();
    assert!(bytes.is_empty(), "expected empty diff, got {} bytes", bytes.len());
}

// --- diff: after changes returns non-empty ---

#[test]
fn diff_after_change_returns_bytes() {
    let mut s = Screen::new(10, 5);
    s.put(0, 0, 'A');
    let bytes = s.diff();
    assert!(!bytes.is_empty(), "expected non-empty diff after put");
}

// --- diff: second diff after sync returns empty ---

#[test]
fn second_diff_returns_empty_after_sync() {
    let mut s = Screen::new(10, 5);
    s.put(1, 2, 'B');
    let _ = s.diff(); // first diff syncs front <- back
    let bytes = s.diff(); // no change since last sync
    assert!(bytes.is_empty(), "expected empty second diff, got {} bytes", bytes.len());
}

// --- diff: clear_back then modify ---

#[test]
fn clear_back_then_modify_shows_change() {
    let mut s = Screen::new(10, 5);
    // Put something, sync it
    s.put(0, 0, 'C');
    let _ = s.diff();

    // Clear back (becomes all spaces) — front still has 'C' at (0,0)
    s.clear_back();
    // Now back != front at (0,0), so diff must be non-empty
    let bytes = s.diff();
    assert!(!bytes.is_empty(), "expected diff after clear_back changed a cell");
}

// --- diff: styled cell produces ANSI sequences ---

#[test]
fn diff_styled_cell_contains_ansi_escape() {
    let mut s = Screen::new(10, 5);
    s.put_styled(0, 0, 'X', Some(31), true); // red + bold
    let bytes = s.diff();
    let text = String::from_utf8_lossy(&bytes);
    // Should contain ESC [ sequences for bold and/or color
    assert!(text.contains('\x1b'), "expected ANSI escape in diff output");
}

// --- diff: plain cell does NOT produce style sequences ---

#[test]
fn diff_plain_cell_no_style_sequences() {
    let mut s = Screen::new(10, 5);
    s.put(0, 0, 'Y');
    let bytes = s.diff();
    let text = String::from_utf8_lossy(&bytes);
    // Must contain the cursor-move ESC sequence, but NOT bold/color codes
    // ESC [ 1 m = bold, ESC [ 3 x m = color
    assert!(!text.contains("\x1b[1m"), "unexpected bold in plain cell");
    // Check no color code like ESC[3_m or ESC[9_m
    let no_color = !text.contains("\x1b[30m")
        && !text.contains("\x1b[31m")
        && !text.contains("\x1b[32m");
    assert!(no_color, "unexpected color in plain cell");
}

// --- clear_back resets back buffer ---

#[test]
fn clear_back_resets_cells() {
    let mut s = Screen::new(5, 5);
    s.put(2, 2, 'Q');
    s.clear_back();
    assert_eq!(s.get(2, 2), Cell::default());
}
