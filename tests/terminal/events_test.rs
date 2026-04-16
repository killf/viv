use viv::terminal::events::*;
use viv::terminal::input::KeyEvent;
use viv::terminal::size::TermSize;

#[test]
fn event_loop_creation() {
    let el = EventLoop::new();
    assert!(el.is_ok());
    // Drop restores stdin flags
}

#[test]
fn poll_timeout_returns_tick() {
    let mut el = EventLoop::new().unwrap();
    let events = el.poll(1).unwrap(); // 1ms timeout
    // Either empty or Tick (no stdin data, no signal)
    for e in &events {
        assert!(matches!(e, Event::Tick));
    }
}

#[test]
fn event_debug_and_eq() {
    let a = Event::Tick;
    let b = Event::Tick;
    assert_eq!(a, b);
    let _ = format!("{:?}", Event::Resize(TermSize { cols: 80, rows: 24 }));
    let _ = format!("{:?}", Event::Key(KeyEvent::Enter));
}
