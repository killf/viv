use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::glob::GlobTool;
use viv::tools::poll_to_completion;

fn json_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_glob_{}", nanos()));
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
fn glob_star_matches_extension() {
    let dir = tempdir();
    fs::write(dir.join("a.rs"), "").unwrap();
    fs::write(dir.join("b.rs"), "").unwrap();
    fs::write(dir.join("c.txt"), "").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"*.rs","path":"{}"}}"#,
        json_path(&dir)
    ))
    .unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
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
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"**/*.rs","path":"{}"}}"#,
        json_path(&dir)
    ))
    .unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
    assert!(result.contains("deep.rs"));
    assert!(result.contains("top.rs"));
}

#[test]
fn glob_results_sorted_by_modification_time() {
    let dir = tempdir();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    let c = dir.join("c.txt");
    fs::write(&a, "a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&b, "b").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&c, "c").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&a, "a updated").unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"*.txt","path":"{}"}}"#, json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert!(lines[0].ends_with("a.txt"), "Expected a.txt first (most recent), got: {}", lines[0]);
}

#[test]
fn glob_ignores_git_directory() {
    let dir = tempdir();
    fs::create_dir_all(dir.join(".git/objects")).unwrap();
    fs::write(dir.join(".git/objects/abc"), "git object").unwrap();
    fs::write(dir.join("real.txt"), "real").unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"**/*","path":"{}"}}"#, json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
    assert!(!result.contains(".git"), "Should not include .git contents");
    assert!(result.contains("real.txt"));
}
