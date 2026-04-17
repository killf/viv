use viv::agent::message::{ContentBlock, Message, SystemBlock, hash_str};
use viv::json::ToJson;

#[test]
fn text_block_serializes_correctly() {
    let b = ContentBlock::Text("hello world".into());
    let json = b.to_json();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("hello world"));
}

#[test]
fn tool_use_block_serializes_correctly() {
    use viv::json::JsonValue;
    let b = ContentBlock::ToolUse {
        id: "tu_01".into(),
        name: "bash".into(),
        input: JsonValue::parse("{\"command\":\"ls\"}").unwrap(),
    };
    let json = b.to_json();
    assert!(json.contains("\"type\":\"tool_use\""));
    assert!(json.contains("\"name\":\"bash\""));
    assert!(json.contains("\"id\":\"tu_01\""));
}

#[test]
fn tool_result_serializes_correctly() {
    let b = ContentBlock::ToolResult {
        tool_use_id: "tu_01".into(),
        content: vec![ContentBlock::Text("output".into())],
        is_error: false,
    };
    let json = b.to_json();
    assert!(json.contains("\"type\":\"tool_result\""));
    assert!(json.contains("\"tool_use_id\":\"tu_01\""));
    assert!(json.contains("\"is_error\":false"));
}

#[test]
fn user_message_role_is_user() {
    let m = Message::user_text("hi");
    assert_eq!(m.role(), "user");
}

#[test]
fn assistant_message_serializes_correctly() {
    let m = Message::Assistant(vec![ContentBlock::Text("answer".into())]);
    let json = m.to_json();
    assert!(json.contains("\"role\":\"assistant\""));
}

#[test]
fn system_block_cached_has_cache_control() {
    let b = SystemBlock::cached("base prompt");
    let json = b.to_json();
    assert!(json.contains("\"cache_control\""));
    assert!(json.contains("\"ephemeral\""));
}

#[test]
fn system_block_dynamic_has_no_cache_control() {
    let b = SystemBlock::dynamic("memory");
    let json = b.to_json();
    assert!(!json.contains("\"cache_control\""));
}

#[test]
fn hash_str_is_deterministic() {
    assert_eq!(hash_str("hello"), hash_str("hello"));
    assert_ne!(hash_str("hello"), hash_str("world"));
}
