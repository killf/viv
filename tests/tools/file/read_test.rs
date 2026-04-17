use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::read::ReadTool;
use viv::tools::poll_to_completion;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_read_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn read_returns_content_with_line_numbers() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "alpha\nbeta\ngamma\n").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}"}}"#, path.display())).unwrap();
    let result = poll_to_completion(ReadTool.execute(&input)).unwrap();
    assert!(result.contains("alpha"));
    assert!(result.contains("beta"));
    assert!(result.contains('1'));  // line number
}

#[test]
fn read_with_offset_skips_lines() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a\nb\nc\n").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}","offset":2,"limit":1}}"#, path.display())).unwrap();
    let result = poll_to_completion(ReadTool.execute(&input)).unwrap();
    assert!(result.contains('b'));
    assert!(!result.contains('a'));
    assert!(!result.contains('c'));
}

#[test]
fn read_missing_file_is_error() {
    let input = JsonValue::parse(r#"{"file_path":"/nonexistent/no.txt"}"#).unwrap();
    assert!(poll_to_completion(ReadTool.execute(&input)).is_err());
}
