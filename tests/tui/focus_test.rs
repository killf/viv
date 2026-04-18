use viv::tui::focus::{FocusManager, UIMode};

#[test]
fn initial_state() {
    let fm = FocusManager::new();
    assert_eq!(fm.mode(), UIMode::Normal);
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn enter_browse_mode() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    assert_eq!(fm.mode(), UIMode::Browse);
}

#[test]
fn exit_browse_mode() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.exit_browse();
    assert_eq!(fm.mode(), UIMode::Normal);
}

#[test]
fn next_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.next();
    assert_eq!(fm.focus_index(), 1);
    fm.next();
    assert_eq!(fm.focus_index(), 2);
    fm.next();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn prev_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.prev();
    assert_eq!(fm.focus_index(), 2);
}

#[test]
fn zero_focusable_stays_at_zero() {
    let mut fm = FocusManager::new();
    fm.enter_browse(0);
    fm.next();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn is_focused() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    assert!(fm.is_focused(0));
    assert!(!fm.is_focused(1));
    fm.next();
    assert!(fm.is_focused(1));
}

#[test]
fn normal_mode_nothing_focused() {
    let fm = FocusManager::new();
    assert!(!fm.is_focused(0));
}

#[test]
fn update_count_clamps_index() {
    let mut fm = FocusManager::new();
    fm.enter_browse(5);
    fm.next();
    fm.next();
    fm.next();
    fm.next();
    assert_eq!(fm.focus_index(), 4);
    fm.update_count(3);
    assert_eq!(fm.focus_index(), 2);
}
