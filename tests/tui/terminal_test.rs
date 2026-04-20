use viv::core::terminal::input::KeyEvent;
use viv::tui::input::InputMode;
use viv::tui::terminal::{EditAction, LineEditor};

fn press(editor: &mut LineEditor, ch: char) -> EditAction {
    editor.handle_key(KeyEvent::Char(ch))
}

fn submit(editor: &mut LineEditor) -> EditAction {
    editor.handle_key(KeyEvent::Enter)
}

fn ctrl_c(editor: &mut LineEditor) -> EditAction {
    editor.handle_key(KeyEvent::CtrlC)
}

// ── Mode detection ────────────────────────────────────────────────────────────

#[test]
fn mode_defaults_to_chat() {
    let editor = LineEditor::new();
    assert_eq!(editor.mode, InputMode::Chat);
}

#[test]
fn typing_slash_as_first_char_switches_to_slash_mode() {
    let mut editor = LineEditor::new();
    assert_eq!(editor.mode, InputMode::Chat);
    let _ = press(&mut editor, '/');
    assert_eq!(editor.mode, InputMode::SlashCommand);
}

#[test]
fn typing_colon_as_first_char_switches_to_colon_mode() {
    let mut editor = LineEditor::new();
    assert_eq!(editor.mode, InputMode::Chat);
    let _ = press(&mut editor, ':');
    assert_eq!(editor.mode, InputMode::ColonCommand);
}

#[test]
fn typing_regular_char_does_not_change_mode() {
    let mut editor = LineEditor::new();
    let _ = press(&mut editor, 'h');
    assert_eq!(editor.mode, InputMode::Chat);
}

#[test]
fn slash_mode_only_when_line_is_empty() {
    let mut editor = LineEditor::new();
    // Type 'a' first, then try '/'
    let _ = press(&mut editor, 'a');
    assert_eq!(editor.mode, InputMode::Chat);
    // '/' on a non-empty line should NOT switch mode
    let _ = press(&mut editor, '/');
    assert_eq!(editor.mode, InputMode::Chat);
}

#[test]
fn ctrl_c_resets_mode_to_chat() {
    let mut editor = LineEditor::new();
    let _ = press(&mut editor, '/');
    assert_eq!(editor.mode, InputMode::SlashCommand);
    let _ = ctrl_c(&mut editor);
    assert_eq!(editor.mode, InputMode::Chat);
}

// ── Submission with mode ──────────────────────────────────────────────────────

#[test]
fn submit_in_chat_mode_returns_continue() {
    let mut editor = LineEditor::new();
    let _ = press(&mut editor, 'h');
    let _ = press(&mut editor, 'i');
    let result = submit(&mut editor);
    match result {
        EditAction::Submit(content) => assert_eq!(content, "hi"),
        other => panic!("expected Submit, got {:?}", other),
    }
}

#[test]
fn submit_in_slash_mode_returns_continue() {
    let mut editor = LineEditor::new();
    let _ = press(&mut editor, '/');
    let _ = press(&mut editor, 'f');
    let _ = press(&mut editor, 'o');
    let _ = press(&mut editor, 'o');
    let result = submit(&mut editor);
    match result {
        EditAction::Submit(content) => assert_eq!(content, "/foo"),
        other => panic!("expected Submit, got {:?}", other),
    }
}

#[test]
fn submit_in_colon_mode_returns_continue() {
    let mut editor = LineEditor::new();
    let _ = press(&mut editor, ':');
    let _ = press(&mut editor, 'w');
    let _ = press(&mut editor, 'q');
    let result = submit(&mut editor);
    match result {
        EditAction::Submit(content) => assert_eq!(content, ":wq"),
        other => panic!("expected Submit, got {:?}", other),
    }
}

#[test]
fn submit_empty_returns_submit() {
    let mut editor = LineEditor::new();
    let result = submit(&mut editor);
    match result {
        EditAction::Submit(content) => assert_eq!(content, ""),
        other => panic!("expected Submit, got {:?}", other),
    }
}

// ── Prompt via mode ──────────────────────────────────────────────────────────

#[test]
fn chat_mode_prompt() {
    assert_eq!(InputMode::Chat.prompt(), "\u{276F} ");
}

#[test]
fn slash_mode_prompt() {
    assert_eq!(InputMode::SlashCommand.prompt(), "/ ");
}

#[test]
fn colon_mode_prompt() {
    assert_eq!(InputMode::ColonCommand.prompt(), ": ");
}
