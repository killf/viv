//! Unit tests for the log module.

#[test]
fn module_name_from_src_path() {
    assert_eq!(viv::log::extract_module("src/agent/mod.rs"), "agent");
}

#[test]
fn module_name_from_src_file() {
    assert_eq!(viv::log::extract_module("src/llm.rs"), "llm");
}

#[test]
fn module_name_from_deep_path() {
    assert_eq!(
        viv::log::extract_module("src/core/runtime/mod.rs"),
        "core.runtime"
    );
}

#[test]
fn level_ordering() {
    assert!(viv::log::Level::Error < viv::log::Level::Warn);
    assert!(viv::log::Level::Warn < viv::log::Level::Info);
    assert!(viv::log::Level::Info < viv::log::Level::Debug);
    assert!(viv::log::Level::Debug < viv::log::Level::Trace);
}

#[test]
fn level_from_str() {
    assert_eq!("ERROR".parse(), Ok(viv::log::Level::Error));
    assert_eq!("WARN".parse(), Ok(viv::log::Level::Warn));
    assert_eq!("INFO".parse(), Ok(viv::log::Level::Info));
    assert_eq!("DEBUG".parse(), Ok(viv::log::Level::Debug));
    assert_eq!("TRACE".parse(), Ok(viv::log::Level::Trace));
    assert!("BAD".parse::<viv::log::Level>().is_err());
}

#[test]
fn record_format_contains_fields() {
    use viv::log::{Level, Record};
    let rec = Record::new(
        Level::Info,
        "test".to_string(),
        "src/test.rs",
        42,
        "hello world".into(),
    );
    let s = rec.to_string();
    assert!(s.contains("hello world"));
    assert!(s.contains("INFO"));
    assert!(s.contains("[test]"));
    assert!(s.contains("src/test.rs:42"));
}

#[test]
fn record_format_time_is_rfc3339() {
    use viv::log::{Level, Record};
    let rec = Record::new(
        Level::Info,
        "test".to_string(),
        "src/test.rs",
        42,
        "msg".into(),
    );
    let s = rec.to_string();
    assert!(s.starts_with("20"));
    assert!(s.contains('T'));
    assert!(s.contains('Z'));
    assert!(s.ends_with('\n'));
}
