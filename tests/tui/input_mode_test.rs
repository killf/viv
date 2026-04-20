use viv::tui::input::InputMode;

// ── InputMode ─────────────────────────────────────────────────────────────────

#[test]
fn input_mode_chat_prompt() {
    assert_eq!(InputMode::Chat.prompt(), "\u{276F} ");
}

#[test]
fn input_mode_slash_command_prompt() {
    assert_eq!(InputMode::SlashCommand.prompt(), "/ ");
}

#[test]
fn input_mode_colon_command_prompt() {
    assert_eq!(InputMode::ColonCommand.prompt(), ": ");
}

#[test]
fn input_mode_equality() {
    assert_eq!(InputMode::Chat, InputMode::Chat);
    assert_eq!(InputMode::SlashCommand, InputMode::SlashCommand);
    assert_eq!(InputMode::ColonCommand, InputMode::ColonCommand);
    assert_ne!(InputMode::Chat, InputMode::SlashCommand);
    assert_ne!(InputMode::Chat, InputMode::ColonCommand);
    assert_ne!(InputMode::SlashCommand, InputMode::ColonCommand);
}
