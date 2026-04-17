use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::bash::{BashBackgroundTool, BashTool};

#[test]
fn bash_captures_stdout() {
    let input = JsonValue::parse(r#"{"command":"echo hello"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("hello"));
}

#[test]
fn bash_captures_stderr() {
    let input = JsonValue::parse(r#"{"command":"echo error >&2"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("error"));
}

#[test]
fn bash_nonzero_exit_code_in_output() {
    let input = JsonValue::parse(r#"{"command":"exit 42"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("42"));
}

#[test]
fn bash_timeout_returns_error() {
    let input = JsonValue::parse(r#"{"command":"sleep 10","timeout_ms":100}"#).unwrap();
    assert!(BashTool.execute(&input).is_err());
}

#[test]
fn bash_background_returns_pid_line() {
    let input = JsonValue::parse(r#"{"command":"sleep 1","description":"test sleep"}"#).unwrap();
    let result = BashBackgroundTool.execute(&input).unwrap();
    assert!(result.contains("pid"));
}
