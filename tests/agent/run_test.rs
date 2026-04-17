use viv::agent::message::{Message, ContentBlock, ToJson};

#[test]
fn message_sequence_builds_correctly() {
    let mut messages: Vec<Message> = vec![];
    messages.push(Message::user_text("hello"));
    assert_eq!(messages[0].role(), "user");
    messages.push(Message::Assistant(vec![ContentBlock::Text("world".into())]));
    assert_eq!(messages[1].role(), "assistant");
    assert_eq!(messages.len(), 2);
}

#[test]
fn tool_result_is_user_role() {
    let tr = Message::User(vec![ContentBlock::ToolResult {
        tool_use_id: "tu_01".into(),
        content: vec![ContentBlock::Text("ok".into())],
        is_error: false,
    }]);
    assert_eq!(tr.role(), "user");
    let json = tr.to_json();
    assert!(json.contains("\"role\":\"user\""));
}
