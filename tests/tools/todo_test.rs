use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::poll_to_completion;
use viv::tools::todo::{TodoReadTool, TodoWriteTool};

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_todo_{}", nanos()));
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
fn todo_write_then_read_roundtrip() {
    let dir = tempdir();
    let path = dir.join("todo.json");
    let write = TodoWriteTool::new(path.clone());
    let input =
        JsonValue::parse(r#"{"todos":[{"id":"1","content":"buy milk","status":"pending"}]}"#)
            .unwrap();
    poll_to_completion(write.execute(&input)).unwrap();

    let read = TodoReadTool::new(path);
    let result = poll_to_completion(read.execute(&JsonValue::Object(vec![]))).unwrap();
    assert!(result.contains("buy milk"));
    assert!(result.contains("pending"));
}

#[test]
fn todo_read_returns_empty_array_when_no_file() {
    let path = std::path::PathBuf::from("/nonexistent/viv_todo_missing.json");
    let read = TodoReadTool::new(path);
    let result = poll_to_completion(read.execute(&JsonValue::Object(vec![]))).unwrap();
    assert_eq!(result, "[]");
}
