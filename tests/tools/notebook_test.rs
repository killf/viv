use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::notebook::NotebookEditTool;
use viv::tools::poll_to_completion;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_nb_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}
fn json_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

const SAMPLE_NOTEBOOK: &str = r##"{"cells":[{"cell_type":"code","source":["print('hello')\n"],"metadata":{},"outputs":[],"id":"cell1"},{"cell_type":"markdown","source":["# Title\n"],"metadata":{},"id":"cell2"}],"metadata":{},"nbformat":4,"nbformat_minor":5}"##;

#[test]
fn notebook_replace_cell_by_id() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell1","new_source":"print('world')"}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.to_lowercase().contains("replace"), "Got: {}", result);
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("world"));
    assert!(!content.contains("hello"));
}

#[test]
fn notebook_insert_cell() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell1","edit_mode":"insert","cell_type":"code","new_source":"x = 42"}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.to_lowercase().contains("insert"), "Got: {}", result);
    let content = fs::read_to_string(&path).unwrap();
    let parsed = JsonValue::parse(&content).unwrap();
    let cells = parsed.get("cells").unwrap().as_array().unwrap();
    assert_eq!(cells.len(), 3);
}

#[test]
fn notebook_delete_cell() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell2","edit_mode":"delete","new_source":""}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.to_lowercase().contains("delete"), "Got: {}", result);
    let content = fs::read_to_string(&path).unwrap();
    let parsed = JsonValue::parse(&content).unwrap();
    let cells = parsed.get("cells").unwrap().as_array().unwrap();
    assert_eq!(cells.len(), 1);
}
