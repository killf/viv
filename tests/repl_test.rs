use viv::repl::{LineEditor, EditAction};
use viv::terminal::input::KeyEvent;

#[test]
fn insert_char() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('h'));
    ed.handle_key(KeyEvent::Char('i'));
    assert_eq!(ed.lines[ed.row], "hi");
    assert_eq!(ed.col, 2);
}

#[test]
fn backspace() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::Char('b'));
    ed.handle_key(KeyEvent::Backspace);
    assert_eq!(ed.lines[ed.row], "a");
    assert_eq!(ed.col, 1);
}

#[test]
fn backspace_at_start() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Backspace);
    assert_eq!(ed.lines[0], "");
    assert_eq!(ed.col, 0);
}

#[test]
fn cursor_movement() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "hello".into();
    ed.col = 5;
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.col, 4);
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.col, 3);
    ed.handle_key(KeyEvent::Right);
    assert_eq!(ed.col, 4);
}

#[test]
fn home_end() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "hello".into();
    ed.col = 3;
    ed.handle_key(KeyEvent::Home);
    assert_eq!(ed.col, 0);
    ed.handle_key(KeyEvent::End);
    assert_eq!(ed.col, 5);
}

#[test]
fn delete_key() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "hello".into();
    ed.col = 2;
    ed.handle_key(KeyEvent::Delete);
    assert_eq!(ed.lines[0], "helo");
    assert_eq!(ed.col, 2);
}

#[test]
fn insert_in_middle() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "hllo".into();
    ed.col = 1;
    ed.handle_key(KeyEvent::Char('e'));
    assert_eq!(ed.lines[0], "hello");
    assert_eq!(ed.col, 2);
}

#[test]
fn left_at_start_does_nothing() {
    let mut ed = LineEditor::new();
    ed.col = 0;
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.col, 0);
}

#[test]
fn right_at_end_does_nothing() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "hi".into();
    ed.col = 2;
    ed.handle_key(KeyEvent::Right);
    assert_eq!(ed.col, 2);
}

#[test]
fn enter_returns_submit() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('h'));
    ed.handle_key(KeyEvent::Char('i'));
    let action = ed.handle_key(KeyEvent::Enter);
    assert_eq!(action, EditAction::Submit("hi".into()));
    assert_eq!(ed.lines, vec!["".to_string()]);
}

#[test]
fn ctrl_d_empty_exits() {
    let mut ed = LineEditor::new();
    let action = ed.handle_key(KeyEvent::CtrlD);
    assert_eq!(action, EditAction::Exit);
}

#[test]
fn ctrl_d_nonempty_continues() {
    let mut ed = LineEditor::new();
    ed.lines[0] = "x".into();
    ed.col = 1;
    let action = ed.handle_key(KeyEvent::CtrlD);
    assert_eq!(action, EditAction::Continue);
}

#[test]
fn ctrl_c_interrupts() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('x'));
    let action = ed.handle_key(KeyEvent::CtrlC);
    assert_eq!(action, EditAction::Interrupt);
}

#[test]
fn shift_enter_inserts_new_line() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('b'));
    assert_eq!(ed.lines, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(ed.row, 1);
    assert_eq!(ed.col, 1);
}

#[test]
fn enter_submits_all_lines_joined() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('b'));
    let action = ed.handle_key(KeyEvent::Enter);
    assert_eq!(action, EditAction::Submit("a\nb".to_string()));
    assert_eq!(ed.lines, vec!["".to_string()]);
    assert_eq!(ed.row, 0);
    assert_eq!(ed.col, 0);
}

#[test]
fn backspace_at_line_start_merges_with_previous() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Backspace); // col=0, merges with previous
    assert_eq!(ed.lines, vec!["a".to_string()]);
    assert_eq!(ed.row, 0);
    assert_eq!(ed.col, 1);
}

#[test]
fn cursor_offset_in_multiline() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::Char('b'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('c'));
    // "ab\nc", cursor after 'c' → offset = 2 (ab) + 1 (\n) + 1 (c) = 4
    assert_eq!(ed.cursor_offset(), 4);
}
