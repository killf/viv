use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::write::WriteTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_write_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn write_creates_file_with_content() {
    let dir = tempdir();
    let path = dir.join("out.txt");
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"hello world"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
}

#[test]
fn write_creates_parent_directories() {
    let dir = tempdir();
    let path = dir.join("a/b/c.txt");
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"hi"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert!(path.exists());
}

#[test]
fn write_overwrites_existing_file() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "old").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"new"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
}
