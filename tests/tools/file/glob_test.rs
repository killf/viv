use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::glob::GlobTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_glob_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn glob_star_matches_extension() {
    let dir = tempdir();
    fs::write(dir.join("a.rs"), "").unwrap();
    fs::write(dir.join("b.rs"), "").unwrap();
    fs::write(dir.join("c.txt"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"pattern":"*.rs","path":"{}"}}"#, dir.display())).unwrap();
    let result = GlobTool.execute(&input).unwrap();
    assert!(result.contains("a.rs"));
    assert!(result.contains("b.rs"));
    assert!(!result.contains("c.txt"));
}

#[test]
fn glob_double_star_recurses() {
    let dir = tempdir();
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("deep.rs"), "").unwrap();
    fs::write(dir.join("top.rs"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"pattern":"**/*.rs","path":"{}"}}"#, dir.display())).unwrap();
    let result = GlobTool.execute(&input).unwrap();
    assert!(result.contains("deep.rs"));
    assert!(result.contains("top.rs"));
}
