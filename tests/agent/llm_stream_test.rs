use viv::agent::message::ContentBlock;
use viv::llm::parse_agent_stream_pub;

#[test]
fn parses_text_delta_from_sse() {
    let sse = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let raw = sse.as_bytes().to_vec();
    let mut collected = String::new();
    let result = parse_agent_stream_pub(&raw, Some(0), &mut |t| collected.push_str(t)).unwrap();
    assert_eq!(collected, "hello");
    assert_eq!(result.text_blocks.len(), 1);
    assert!(result.tool_uses.is_empty());
}

#[test]
fn parses_tool_use_from_sse() {
    let sse = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_01\",\"name\":\"bash\",\"input\":{}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls\\\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"}\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let raw = sse.as_bytes().to_vec();
    let result = parse_agent_stream_pub(&raw, Some(0), &mut |_| {}).unwrap();
    assert_eq!(result.tool_uses.len(), 1);
    if let ContentBlock::ToolUse { name, .. } = &result.tool_uses[0] {
        assert_eq!(name, "bash");
    } else {
        panic!("expected ToolUse");
    }
}
