use std::future::Future;
use std::pin::Pin;
use viv::core::json::JsonValue;
use viv::tools::{PermissionLevel, Tool, ToolRegistry, poll_to_completion};

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "Echoes input text"
    }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(
            r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
        )
        .unwrap()
    }
    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = viv::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
            Ok(input
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string())
        })
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
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
    let parsed = JsonValue::parse(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "echo");
    assert!(arr[0].get("description").is_some());
    assert!(arr[0].get("input_schema").is_some());
}

#[test]
fn tool_execute_returns_input_text() {
    let input = JsonValue::parse(r#"{"text":"hello"}"#).unwrap();
    assert_eq!(
        poll_to_completion(EchoTool.execute(&input)).unwrap(),
        "hello"
    );
}

struct QuoteTool;

impl Tool for QuoteTool {
    fn name(&self) -> &str {
        "quote"
    }
    fn description(&self) -> &str {
        "Handles \"quotes\" and \\backslashes"
    }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }
    fn execute(
        &self,
        _input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = viv::Result<String>> + Send + '_>> {
        Box::pin(async { Ok(String::new()) })
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

#[test]
fn to_api_json_escapes_special_characters() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(QuoteTool));
    let json = reg.to_api_json();
    let parsed = JsonValue::parse(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "quote");
    assert!(
        arr[0]
            .get("description")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("\"quotes\"")
    );
}

#[test]
fn default_tools_have_claude_code_names() {
    let llm_config = viv::llm::LLMConfig::from_env(&viv::config::ModelConfig::default());
    if llm_config.is_err() {
        return;
    }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let reg = ToolRegistry::default_tools(llm);
    assert!(reg.get("Read").is_some(), "Expected 'Read' tool");
    assert!(reg.get("Write").is_some(), "Expected 'Write' tool");
    assert!(reg.get("Edit").is_some(), "Expected 'Edit' tool");
    assert!(reg.get("FileRead").is_none(), "FileRead should be renamed");
    assert!(
        reg.get("FileWrite").is_none(),
        "FileWrite should be renamed"
    );
    assert!(reg.get("FileEdit").is_none(), "FileEdit should be renamed");
}
