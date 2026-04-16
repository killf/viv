use viv::repl::*;
use viv::terminal::input::KeyEvent;

#[test]
fn insert_char() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('h'));
    ed.handle_key(KeyEvent::Char('i'));
    assert_eq!(ed.buf, "hi");
    assert_eq!(ed.cursor, 2);
}

#[test]
fn backspace() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::Char('b'));
    ed.handle_key(KeyEvent::Backspace);
    assert_eq!(ed.buf, "a");
    assert_eq!(ed.cursor, 1);
}

#[test]
fn backspace_at_start() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Backspace);
    assert_eq!(ed.buf, "");
    assert_eq!(ed.cursor, 0);
}

#[test]
fn cursor_movement() {
    let mut ed = LineEditor::new();
    ed.buf = "hello".into();
    ed.cursor = 5;
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.cursor, 4);
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.cursor, 3);
    ed.handle_key(KeyEvent::Right);
    assert_eq!(ed.cursor, 4);
}

#[test]
fn home_end() {
    let mut ed = LineEditor::new();
    ed.buf = "hello".into();
    ed.cursor = 3;
    ed.handle_key(KeyEvent::Home);
    assert_eq!(ed.cursor, 0);
    ed.handle_key(KeyEvent::End);
    assert_eq!(ed.cursor, 5);
}

#[test]
fn delete_key() {
    let mut ed = LineEditor::new();
    ed.buf = "hello".into();
    ed.cursor = 2;
    ed.handle_key(KeyEvent::Delete);
    assert_eq!(ed.buf, "helo");
    assert_eq!(ed.cursor, 2);
}

#[test]
fn insert_in_middle() {
    let mut ed = LineEditor::new();
    ed.buf = "hllo".into();
    ed.cursor = 1;
    ed.handle_key(KeyEvent::Char('e'));
    assert_eq!(ed.buf, "hello");
    assert_eq!(ed.cursor, 2);
}

#[test]
fn left_at_start_does_nothing() {
    let mut ed = LineEditor::new();
    ed.cursor = 0;
    ed.handle_key(KeyEvent::Left);
    assert_eq!(ed.cursor, 0);
}

#[test]
fn right_at_end_does_nothing() {
    let mut ed = LineEditor::new();
    ed.buf = "hi".into();
    ed.cursor = 2;
    ed.handle_key(KeyEvent::Right);
    assert_eq!(ed.cursor, 2);
}

#[test]
fn enter_returns_submit() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('h'));
    ed.handle_key(KeyEvent::Char('i'));
    let action = ed.handle_key(KeyEvent::Enter);
    assert_eq!(action, EditAction::Submit("hi".into()));
    assert_eq!(ed.buf, "");
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
    ed.buf = "x".into();
    ed.cursor = 1;
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
