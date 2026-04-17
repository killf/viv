use viv::core::json::JsonValue;
use viv::tools::{PermissionLevel, Tool, ToolRegistry};

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "Echoes input text" }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#).unwrap()
    }
    fn execute(&self, input: &JsonValue) -> viv::Result<String> {
        Ok(input.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

#[test]
fn registry_get_returns_registered_tool() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(EchoTool));
    assert!(reg.get("echo").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn registry_to_api_json_has_required_fields() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(EchoTool));
    let json = reg.to_api_json();
    assert!(json.contains("\"name\":\"echo\""));
    assert!(json.contains("\"description\""));
    assert!(json.contains("\"input_schema\""));
}

#[test]
fn tool_execute_returns_input_text() {
    let input = JsonValue::parse(r#"{"text":"hello"}"#).unwrap();
    assert_eq!(EchoTool.execute(&input).unwrap(), "hello");
}
