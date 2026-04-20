use viv::core::terminal::input::{InputEvent, InputParser, KeyEvent, MouseEvent};

fn parse_single(bytes: &[u8]) -> Option<InputEvent> {
    let mut parser = InputParser::new();
    parser.feed(bytes);
    parser.next_event()
}

#[test]
fn test_ascii_char() {
    assert_eq!(parse_single(b"a"), Some(InputEvent::Key(KeyEvent::Char('a'))));
    assert_eq!(parse_single(b"z"), Some(InputEvent::Key(KeyEvent::Char('z'))));
    assert_eq!(parse_single(b"A"), Some(InputEvent::Key(KeyEvent::Char('A'))));
    assert_eq!(parse_single(b" "), Some(InputEvent::Key(KeyEvent::Char(' '))));
    assert_eq!(parse_single(b"~"), Some(InputEvent::Key(KeyEvent::Char('~'))));
}

#[test]
fn test_enter() {
    assert_eq!(parse_single(&[13]), Some(InputEvent::Key(KeyEvent::Enter)));
}

#[test]
fn test_backspace() {
    assert_eq!(parse_single(&[127]), Some(InputEvent::Key(KeyEvent::Backspace)));
}

#[test]
fn test_ctrl_c() {
    assert_eq!(parse_single(&[3]), Some(InputEvent::Key(KeyEvent::CtrlC)));
}

#[test]
fn test_ctrl_d() {
    assert_eq!(parse_single(&[4]), Some(InputEvent::Key(KeyEvent::CtrlD)));
}

#[test]
fn test_arrow_up() {
    assert_eq!(parse_single(b"\x1b[A"), Some(InputEvent::Key(KeyEvent::Up)));
}

#[test]
fn test_arrow_down() {
    assert_eq!(parse_single(b"\x1b[B"), Some(InputEvent::Key(KeyEvent::Down)));
}

#[test]
fn test_arrow_right() {
    assert_eq!(parse_single(b"\x1b[C"), Some(InputEvent::Key(KeyEvent::Right)));
}

#[test]
fn test_arrow_left() {
    assert_eq!(parse_single(b"\x1b[D"), Some(InputEvent::Key(KeyEvent::Left)));
}

#[test]
fn test_home() {
    assert_eq!(parse_single(b"\x1b[H"), Some(InputEvent::Key(KeyEvent::Home)));
}

#[test]
fn test_end() {
    assert_eq!(parse_single(b"\x1b[F"), Some(InputEvent::Key(KeyEvent::End)));
}

#[test]
fn test_delete() {
    assert_eq!(parse_single(b"\x1b[3~"), Some(InputEvent::Key(KeyEvent::Delete)));
}

#[test]
fn test_escape_alone() {
    assert_eq!(parse_single(b"\x1b"), Some(InputEvent::Key(KeyEvent::Escape)));
}

#[test]
fn test_utf8_char() {
    // '你' is U+4F60, encoded as 0xE4 0xBD 0xA0 in UTF-8
    let bytes: &[u8] = "你".as_bytes();
    assert_eq!(parse_single(bytes), Some(InputEvent::Key(KeyEvent::Char('你'))));
}

#[test]
fn test_utf8_multibyte_various() {
    // '€' is U+20AC, encoded as 0xE2 0x82 0xAC
    assert_eq!(parse_single("€".as_bytes()), Some(InputEvent::Key(KeyEvent::Char('€'))));
    // 'é' is U+00E9, encoded as 0xC3 0xA9
    assert_eq!(parse_single("é".as_bytes()), Some(InputEvent::Key(KeyEvent::Char('é'))));
}

#[test]
fn test_multiple_events_in_one_feed() {
    let mut parser = InputParser::new();
    // Feed "ab\r" — 'a', 'b', Enter
    parser.feed(b"ab\x0d");

    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Char('a'))));
    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Char('b'))));
    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Enter)));
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_multiple_arrow_keys() {
    let mut parser = InputParser::new();
    parser.feed(b"\x1b[A\x1b[B");

    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Up)));
    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Down)));
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_no_event_returns_none() {
    let mut parser = InputParser::new();
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_empty_feed_returns_none() {
    let mut parser = InputParser::new();
    parser.feed(b"");
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_key_event_debug_clone_partialeq() {
    let a = KeyEvent::Char('a');
    let b = a.clone();
    assert_eq!(a, b);
    let _ = format!("{:?}", a);

    let unknown = KeyEvent::Unknown(vec![0x01, 0x02]);
    let unknown2 = unknown.clone();
    assert_eq!(unknown, unknown2);
}

// SGR mouse sequence tests

#[test]
fn test_sgr_mouse_left_press() {
    // ESC [ < 0 ; 10 ; 20 M  — left button press at (10, 20)
    let bytes: &[u8] = b"\x1b[<0;10;20M";
    assert_eq!(parse_single(bytes), Some(InputEvent::Mouse(MouseEvent::LeftPress)));
}

#[test]
fn test_sgr_mouse_left_release() {
    // ESC [ < 0 ; 10 ; 20 m  — left button release at (10, 20)
    let bytes: &[u8] = b"\x1b[<0;10;20m";
    assert_eq!(parse_single(bytes), Some(InputEvent::Mouse(MouseEvent::LeftRelease)));
}

#[test]
fn test_sgr_mouse_wheel_up() {
    // ESC [ < 64 ; 0 ; 0 M  — wheel up
    let bytes: &[u8] = b"\x1b[<64;0;0M";
    assert_eq!(parse_single(bytes), Some(InputEvent::Mouse(MouseEvent::WheelUp)));
}

#[test]
fn test_sgr_mouse_wheel_down() {
    // ESC [ < 65 ; 0 ; 0 M  — wheel down
    let bytes: &[u8] = b"\x1b[<65;0;0M";
    assert_eq!(parse_single(bytes), Some(InputEvent::Mouse(MouseEvent::WheelDown)));
}

#[test]
fn test_sgr_mouse_after_key_event() {
    // Feed a key event followed by a mouse event
    let mut parser = InputParser::new();
    parser.feed(b"a\x1b[<0;5;5M");

    assert_eq!(parser.next_event(), Some(InputEvent::Key(KeyEvent::Char('a'))));
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::LeftPress)));
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_non_mouse_csi_unknown() {
    // ESC [ Z (Shift+Tab) should still be unknown
    let bytes: &[u8] = b"\x1b[Z";
    let result = parse_single(bytes);
    assert!(matches!(result, Some(InputEvent::Key(KeyEvent::Unknown(_)))));
}
