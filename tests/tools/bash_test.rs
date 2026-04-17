use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::bash::BashTool;
use viv::tools::poll_to_completion;

#[test]
fn bash_captures_stdout() {
    let input = JsonValue::parse(r#"{"command":"echo hello"}"#).unwrap();
    let result = poll_to_completion(BashTool.execute(&input)).unwrap();
    assert!(result.contains("hello"));
}

#[test]
fn bash_captures_stderr() {
    let input = JsonValue::parse(r#"{"command":"echo error >&2"}"#).unwrap();
    let result = poll_to_completion(BashTool.execute(&input)).unwrap();
    assert!(result.contains("error"));
}

#[test]
fn bash_nonzero_exit_code_in_output() {
    let input = JsonValue::parse(r#"{"command":"exit 42"}"#).unwrap();
    let result = poll_to_completion(BashTool.execute(&input)).unwrap();
    assert!(result.contains("42"));
}

#[test]
fn bash_timeout_returns_error() {
    let input = JsonValue::parse(r#"{"command":"sleep 10","timeout":100}"#).unwrap();
    assert!(poll_to_completion(BashTool.execute(&input)).is_err());
}

#[test]
fn bash_run_in_background_returns_pid() {
    let input = JsonValue::parse(r#"{"command":"sleep 1","run_in_background":true}"#).unwrap();
    let result = poll_to_completion(BashTool.execute(&input)).unwrap();
    assert!(result.contains("pid"));
}
