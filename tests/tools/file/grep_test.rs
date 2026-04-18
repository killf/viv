use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::grep::GrepTool;
use viv::tools::poll_to_completion;

fn json_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_grep_{}", nanos()));
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
fn grep_files_with_matches_default_mode() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "hello world").unwrap();
    fs::write(dir.join("b.txt"), "goodbye world").unwrap();
    fs::write(dir.join("c.txt"), "nothing here").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"world","path":"{}"}}"#,
        json_path(&dir)
    ))
    .unwrap();
    let result = poll_to_completion(GrepTool.execute(&input)).unwrap();
    assert!(result.contains("a.txt"));
    assert!(result.contains("b.txt"));
    assert!(!result.contains("c.txt"));
}

#[test]
fn grep_content_mode_shows_matching_lines() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "line1\nfoo bar\nline3").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"foo","path":"{}","output_mode":"content"}}"#,
        json_path(&dir)
    ))
    .unwrap();
    let result = poll_to_completion(GrepTool.execute(&input)).unwrap();
    assert!(result.contains("foo bar"));
    assert!(!result.contains("line1"));
}

#[test]
fn grep_glob_filter_limits_files() {
    let dir = tempdir();
    fs::write(dir.join("a.rs"), "fn main() {}").unwrap();
    fs::write(dir.join("b.txt"), "fn main() {}").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"main","path":"{}","glob":"*.rs"}}"#,
        json_path(&dir)
    ))
    .unwrap();
    let result = poll_to_completion(GrepTool.execute(&input)).unwrap();
    assert!(result.contains("a.rs"));
    assert!(!result.contains("b.txt"));
}
