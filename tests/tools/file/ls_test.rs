use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::ls::LsTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_ls_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn ls_shows_files_in_directory() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "").unwrap();
    fs::write(dir.join("b.txt"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"path":"{}"}}"#, dir.display())).unwrap();
    let result = LsTool.execute(&input).unwrap();
    assert!(result.contains("a.txt"));
    assert!(result.contains("b.txt"));
}

#[test]
fn ls_directories_have_trailing_slash() {
    let dir = tempdir();
    fs::create_dir(dir.join("subdir")).unwrap();
    let input = JsonValue::parse(&format!(r#"{{"path":"{}"}}"#, dir.display())).unwrap();
    let result = LsTool.execute(&input).unwrap();
    assert!(result.contains("subdir/"));
}

#[test]
fn ls_no_path_uses_current_dir() {
    let result = LsTool.execute(&JsonValue::Object(vec![])).unwrap();
    assert!(!result.is_empty());
}
