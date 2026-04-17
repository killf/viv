//! Tests for the spinner animation module.
use viv::tui::spinner::*;

#[test]
fn spinner_default_has_multiple_frames() {
    let s = Spinner::new();
    assert!(s.frame_count() >= 4, "spinner should have at least 4 animation frames");
}

#[test]
fn spinner_frame_cycles_over_time() {
    let s = Spinner::new();
    // At t=0 we're on frame 0
    let f0 = s.frame_at(0);
    // After one full frame period, we should be on frame 1
    let f1 = s.frame_at(s.frame_duration_ms());
    assert_ne!(f0, f1, "frame should advance after frame_duration_ms");
}

#[test]
fn spinner_frame_wraps() {
    let s = Spinner::new();
    let total = s.frame_count() as u64 * s.frame_duration_ms();
    // After a full cycle, we're back to frame 0
    assert_eq!(s.frame_at(0), s.frame_at(total));
}

#[test]
fn spinner_frame_duration_is_120ms() {
    let s = Spinner::new();
    assert_eq!(s.frame_duration_ms(), 120);
}

#[test]
fn spinner_frames_are_non_empty_strings() {
    let s = Spinner::new();
    for i in 0..s.frame_count() {
        let frame = s.frame_at(i as u64 * s.frame_duration_ms());
        assert!(!frame.is_empty(), "frame {} should not be empty", i);
    }
}

#[test]
fn random_verb_returns_something() {
    let verb = random_verb(0);
    assert!(!verb.is_empty());
    assert!(verb.chars().last().unwrap().is_alphabetic() || verb.ends_with('g'));
}

#[test]
fn random_verb_is_stable_for_same_seed() {
    let a = random_verb(42);
    let b = random_verb(42);
    assert_eq!(a, b);
}

#[test]
fn random_verbs_differ_across_seeds() {
    // Across 10 different seeds, we should see at least 2 distinct verbs
    let verbs: std::collections::HashSet<_> =
        (0..10u64).map(random_verb).collect();
    assert!(verbs.len() >= 2);
}
