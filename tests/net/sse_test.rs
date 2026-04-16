use viv::net::sse::*;

#[test]
fn parse_simple_event() {
    let mut p = SseParser::new();
    p.feed("event: message_start\ndata: {\"type\":\"message_start\"}\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("message_start"));
    assert_eq!(events[0].data, "{\"type\":\"message_start\"}");
}

#[test]
fn parse_content_block_delta() {
    let mut p = SseParser::new();
    p.feed("event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("content_block_delta"));
    assert!(events[0].data.contains("\"text\":\"Hello\""));
}

#[test]
fn parse_multiple_events() {
    let mut p = SseParser::new();
    p.feed("event: a\ndata: 1\n\nevent: b\ndata: 2\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].data, "1");
    assert_eq!(events[1].data, "2");
}

#[test]
fn partial_event() {
    let mut p = SseParser::new();
    p.feed("event: a\ndata: hel");
    assert_eq!(p.drain().len(), 0);
    p.feed("lo\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello");
}

#[test]
fn multi_line_data() {
    let mut p = SseParser::new();
    p.feed("data: line1\ndata: line2\n\n");
    let events = p.drain();
    assert_eq!(events[0].data, "line1\nline2");
}

#[test]
fn ignore_comments() {
    let mut p = SseParser::new();
    p.feed(": this is a comment\nevent: ping\ndata: ok\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("ping"));
}

#[test]
fn empty_data_not_emitted() {
    let mut p = SseParser::new();
    p.feed("event: ping\n\n");
    let events = p.drain();
    assert_eq!(events.len(), 0);
}
