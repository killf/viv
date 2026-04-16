use viv::terminal::input::{InputParser, KeyEvent};

fn parse_single(bytes: &[u8]) -> Option<KeyEvent> {
    let mut parser = InputParser::new();
    parser.feed(bytes);
    parser.next_event()
}

#[test]
fn test_ascii_char() {
    assert_eq!(parse_single(b"a"), Some(KeyEvent::Char('a')));
    assert_eq!(parse_single(b"z"), Some(KeyEvent::Char('z')));
    assert_eq!(parse_single(b"A"), Some(KeyEvent::Char('A')));
    assert_eq!(parse_single(b" "), Some(KeyEvent::Char(' ')));
    assert_eq!(parse_single(b"~"), Some(KeyEvent::Char('~')));
}

#[test]
fn test_enter() {
    assert_eq!(parse_single(&[13]), Some(KeyEvent::Enter));
}

#[test]
fn test_backspace() {
    assert_eq!(parse_single(&[127]), Some(KeyEvent::Backspace));
}

#[test]
fn test_ctrl_c() {
    assert_eq!(parse_single(&[3]), Some(KeyEvent::CtrlC));
}

#[test]
fn test_ctrl_d() {
    assert_eq!(parse_single(&[4]), Some(KeyEvent::CtrlD));
}

#[test]
fn test_arrow_up() {
    assert_eq!(parse_single(b"\x1b[A"), Some(KeyEvent::Up));
}

#[test]
fn test_arrow_down() {
    assert_eq!(parse_single(b"\x1b[B"), Some(KeyEvent::Down));
}

#[test]
fn test_arrow_right() {
    assert_eq!(parse_single(b"\x1b[C"), Some(KeyEvent::Right));
}

#[test]
fn test_arrow_left() {
    assert_eq!(parse_single(b"\x1b[D"), Some(KeyEvent::Left));
}

#[test]
fn test_home() {
    assert_eq!(parse_single(b"\x1b[H"), Some(KeyEvent::Home));
}

#[test]
fn test_end() {
    assert_eq!(parse_single(b"\x1b[F"), Some(KeyEvent::End));
}

#[test]
fn test_delete() {
    assert_eq!(parse_single(b"\x1b[3~"), Some(KeyEvent::Delete));
}

#[test]
fn test_escape_alone() {
    assert_eq!(parse_single(b"\x1b"), Some(KeyEvent::Escape));
}

#[test]
fn test_utf8_char() {
    // '你' is U+4F60, encoded as 0xE4 0xBD 0xA0 in UTF-8
    let bytes: &[u8] = "你".as_bytes();
    assert_eq!(parse_single(bytes), Some(KeyEvent::Char('你')));
}

#[test]
fn test_utf8_multibyte_various() {
    // '€' is U+20AC, encoded as 0xE2 0x82 0xAC
    assert_eq!(parse_single("€".as_bytes()), Some(KeyEvent::Char('€')));
    // 'é' is U+00E9, encoded as 0xC3 0xA9
    assert_eq!(parse_single("é".as_bytes()), Some(KeyEvent::Char('é')));
}

#[test]
fn test_multiple_events_in_one_feed() {
    let mut parser = InputParser::new();
    // Feed "ab\r" — 'a', 'b', Enter
    parser.feed(b"ab\x0d");

    assert_eq!(parser.next_event(), Some(KeyEvent::Char('a')));
    assert_eq!(parser.next_event(), Some(KeyEvent::Char('b')));
    assert_eq!(parser.next_event(), Some(KeyEvent::Enter));
    assert_eq!(parser.next_event(), None);
}

#[test]
fn test_multiple_arrow_keys() {
    let mut parser = InputParser::new();
    parser.feed(b"\x1b[A\x1b[B");

    assert_eq!(parser.next_event(), Some(KeyEvent::Up));
    assert_eq!(parser.next_event(), Some(KeyEvent::Down));
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
