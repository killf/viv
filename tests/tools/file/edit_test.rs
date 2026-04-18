use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::edit::{EditTool, MultiEditTool};
use viv::tools::poll_to_completion;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_edit_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
}

#[test]
fn edit_replaces_unique_string() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"world","new_string":"rust"}}"#,
        path.display()
    ))
    .unwrap();
    poll_to_completion(EditTool.execute(&input)).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello rust");
}

#[test]
fn edit_fails_when_old_string_not_found() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"missing","new_string":"x"}}"#,
        path.display()
    ))
    .unwrap();
    assert!(poll_to_completion(EditTool.execute(&input)).is_err());
}

#[test]
fn edit_fails_when_old_string_not_unique() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a a a").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"a","new_string":"b"}}"#,
        path.display()
    ))
    .unwrap();
    assert!(poll_to_completion(EditTool.execute(&input)).is_err());
}

#[test]
fn edit_replace_all_replaces_every_occurrence() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a a a").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"a","new_string":"b","replace_all":true}}"#,
        path.display()
    ))
    .unwrap();
    poll_to_completion(EditTool.execute(&input)).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "b b b");
}

#[test]
fn multi_edit_applies_edits_in_order() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world foo").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","edits":[{{"old_string":"hello","new_string":"hi"}},{{"old_string":"foo","new_string":"bar"}}]}}"#,
        path.display()
    )).unwrap();
    poll_to_completion(MultiEditTool.execute(&input)).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hi world bar");
}
