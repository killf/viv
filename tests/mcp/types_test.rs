use viv::core::json::JsonValue;
use viv::mcp::types::*;

// --- ServerCapabilities ---

#[test]
fn parse_server_capabilities_tools_only() {
    let json = JsonValue::parse(r#"{
        "capabilities": {
            "tools": {
                "listChanged": true
            }
        }
    }"#).unwrap();

    let caps = ServerCapabilities::from_json(
        json.get("capabilities").unwrap()
    ).unwrap();

    assert!(caps.tools.is_some());
    assert!(caps.tools.unwrap().list_changed);
    assert!(caps.resources.is_none());
    assert!(caps.prompts.is_none());
}

#[test]
fn parse_server_capabilities_all() {
    let json = JsonValue::parse(r#"{
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "subscribe": true, "listChanged": true },
            "prompts": { "listChanged": true }
        }
    }"#).unwrap();

    let caps = ServerCapabilities::from_json(
        json.get("capabilities").unwrap()
    ).unwrap();

    let tools = caps.tools.unwrap();
    assert!(!tools.list_changed);

    let resources = caps.resources.unwrap();
    assert!(resources.subscribe);
    assert!(resources.list_changed);

    let prompts = caps.prompts.unwrap();
    assert!(prompts.list_changed);
}

#[test]
fn parse_server_capabilities_empty() {
    let json = JsonValue::parse(r#"{}"#).unwrap();
    let caps = ServerCapabilities::from_json(&json).unwrap();
    assert!(caps.tools.is_none());
    assert!(caps.resources.is_none());
    assert!(caps.prompts.is_none());
}

// --- McpTool ---

#[test]
fn parse_mcp_tool() {
    let json = JsonValue::parse(r#"{
        "name": "read_file",
        "description": "Read a file from disk",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }
    }"#).unwrap();

    let tool = McpTool::from_json(&json).unwrap();
    assert_eq!(tool.name, "read_file");
    assert_eq!(tool.description.as_deref(), Some("Read a file from disk"));
    // input_schema should be the object value
    assert!(tool.input_schema.get("type").is_some());
    assert_eq!(tool.input_schema.get("type").unwrap().as_str(), Some("object"));
}

#[test]
fn parse_mcp_tool_no_description() {
    let json = JsonValue::parse(r#"{
        "name": "ping",
        "inputSchema": { "type": "object" }
    }"#).unwrap();

    let tool = McpTool::from_json(&json).unwrap();
    assert_eq!(tool.name, "ping");
    assert!(tool.description.is_none());
}

#[test]
fn parse_mcp_tool_list() {
    let json = JsonValue::parse(r#"{
        "tools": [
            { "name": "a", "inputSchema": { "type": "object" } },
            { "name": "b", "description": "tool b", "inputSchema": { "type": "object" } }
        ]
    }"#).unwrap();

    let tools = McpTool::parse_list(&json).unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "a");
    assert_eq!(tools[1].name, "b");
    assert_eq!(tools[1].description.as_deref(), Some("tool b"));
}

// --- McpResource ---

#[test]
fn parse_mcp_resource() {
    let json = JsonValue::parse(r#"{
        "uri": "file:///tmp/test.txt",
        "name": "test.txt",
        "description": "A test file",
        "mimeType": "text/plain"
    }"#).unwrap();

    let res = McpResource::from_json(&json).unwrap();
    assert_eq!(res.uri, "file:///tmp/test.txt");
    assert_eq!(res.name, "test.txt");
    assert_eq!(res.description.as_deref(), Some("A test file"));
    assert_eq!(res.mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn parse_mcp_resource_minimal() {
    let json = JsonValue::parse(r#"{
        "uri": "mem://data",
        "name": "data"
    }"#).unwrap();

    let res = McpResource::from_json(&json).unwrap();
    assert_eq!(res.uri, "mem://data");
    assert_eq!(res.name, "data");
    assert!(res.description.is_none());
    assert!(res.mime_type.is_none());
}

#[test]
fn parse_mcp_resource_list() {
    let json = JsonValue::parse(r#"{
        "resources": [
            { "uri": "file:///a", "name": "a" },
            { "uri": "file:///b", "name": "b", "mimeType": "text/html" }
        ]
    }"#).unwrap();

    let resources = McpResource::parse_list(&json).unwrap();
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].uri, "file:///a");
    assert_eq!(resources[1].mime_type.as_deref(), Some("text/html"));
}

// --- McpPrompt ---

#[test]
fn parse_mcp_prompt() {
    let json = JsonValue::parse(r#"{
        "name": "summarize",
        "description": "Summarize text",
        "arguments": [
            {
                "name": "text",
                "description": "The text to summarize",
                "required": true
            },
            {
                "name": "style",
                "required": false
            }
        ]
    }"#).unwrap();

    let prompt = McpPrompt::from_json(&json).unwrap();
    assert_eq!(prompt.name, "summarize");
    assert_eq!(prompt.description.as_deref(), Some("Summarize text"));
    assert_eq!(prompt.arguments.len(), 2);
    assert_eq!(prompt.arguments[0].name, "text");
    assert_eq!(prompt.arguments[0].description.as_deref(), Some("The text to summarize"));
    assert!(prompt.arguments[0].required);
    assert_eq!(prompt.arguments[1].name, "style");
    assert!(!prompt.arguments[1].required);
}

#[test]
fn parse_mcp_prompt_no_arguments() {
    let json = JsonValue::parse(r#"{
        "name": "greet"
    }"#).unwrap();

    let prompt = McpPrompt::from_json(&json).unwrap();
    assert_eq!(prompt.name, "greet");
    assert!(prompt.description.is_none());
    assert!(prompt.arguments.is_empty());
}

#[test]
fn parse_mcp_prompt_list() {
    let json = JsonValue::parse(r#"{
        "prompts": [
            { "name": "x" },
            { "name": "y", "description": "prompt y" }
        ]
    }"#).unwrap();

    let prompts = McpPrompt::parse_list(&json).unwrap();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].name, "x");
    assert_eq!(prompts[1].description.as_deref(), Some("prompt y"));
}

// --- ToolCallResult ---

#[test]
fn parse_tool_call_result() {
    let json = JsonValue::parse(r#"{
        "content": [
            { "type": "text", "text": "File contents here" }
        ],
        "isError": false
    }"#).unwrap();

    let result = ToolCallResult::from_json(&json).unwrap();
    assert!(!result.is_error);
    assert_eq!(result.content.len(), 1);
    match &result.content[0] {
        ContentItem::Text(t) => assert_eq!(t, "File contents here"),
        _ => panic!("expected Text content item"),
    }
}

#[test]
fn parse_tool_call_result_error() {
    let json = JsonValue::parse(r#"{
        "content": [
            { "type": "text", "text": "Something went wrong" }
        ],
        "isError": true
    }"#).unwrap();

    let result = ToolCallResult::from_json(&json).unwrap();
    assert!(result.is_error);
}

#[test]
fn parse_tool_call_result_image() {
    let json = JsonValue::parse(r#"{
        "content": [
            { "type": "image", "data": "base64data", "mimeType": "image/png" }
        ]
    }"#).unwrap();

    let result = ToolCallResult::from_json(&json).unwrap();
    assert!(!result.is_error);
    match &result.content[0] {
        ContentItem::Image { data, mime_type } => {
            assert_eq!(data, "base64data");
            assert_eq!(mime_type, "image/png");
        }
        _ => panic!("expected Image content item"),
    }
}

#[test]
fn parse_tool_call_result_resource() {
    let json = JsonValue::parse(r#"{
        "content": [
            { "type": "resource", "resource": { "uri": "file:///x", "text": "hello" } }
        ]
    }"#).unwrap();

    let result = ToolCallResult::from_json(&json).unwrap();
    match &result.content[0] {
        ContentItem::Resource { uri, text } => {
            assert_eq!(uri, "file:///x");
            assert_eq!(text, "hello");
        }
        _ => panic!("expected Resource content item"),
    }
}

#[test]
fn tool_call_result_to_text() {
    let json = JsonValue::parse(r#"{
        "content": [
            { "type": "text", "text": "line1" },
            { "type": "image", "data": "xxx", "mimeType": "image/png" },
            { "type": "text", "text": "line2" },
            { "type": "resource", "resource": { "uri": "file:///x", "text": "res_text" } }
        ]
    }"#).unwrap();

    let result = ToolCallResult::from_json(&json).unwrap();
    assert_eq!(result.to_text(), "line1\nline2\nres_text");
}

#[test]
fn tool_call_result_to_text_empty() {
    let json = JsonValue::parse(r#"{ "content": [] }"#).unwrap();
    let result = ToolCallResult::from_json(&json).unwrap();
    assert_eq!(result.to_text(), "");
}

// --- ResourceContent ---

#[test]
fn parse_resource_content_text() {
    let json = JsonValue::parse(r#"{
        "contents": [
            {
                "uri": "file:///test.txt",
                "mimeType": "text/plain",
                "text": "Hello world"
            }
        ]
    }"#).unwrap();

    let rc = ResourceContent::from_json(&json).unwrap();
    assert_eq!(rc.contents.len(), 1);
    match &rc.contents[0] {
        ResourceContentItem::Text { uri, mime_type, text } => {
            assert_eq!(uri, "file:///test.txt");
            assert_eq!(mime_type.as_deref(), Some("text/plain"));
            assert_eq!(text, "Hello world");
        }
        _ => panic!("expected Text resource content"),
    }
}

#[test]
fn parse_resource_content_blob() {
    let json = JsonValue::parse(r#"{
        "contents": [
            {
                "uri": "file:///image.png",
                "mimeType": "image/png",
                "blob": "base64encodeddata"
            }
        ]
    }"#).unwrap();

    let rc = ResourceContent::from_json(&json).unwrap();
    match &rc.contents[0] {
        ResourceContentItem::Blob { uri, mime_type, blob } => {
            assert_eq!(uri, "file:///image.png");
            assert_eq!(mime_type.as_deref(), Some("image/png"));
            assert_eq!(blob, "base64encodeddata");
        }
        _ => panic!("expected Blob resource content"),
    }
}

// --- PromptMessages ---

#[test]
fn parse_prompt_messages() {
    let json = JsonValue::parse(r#"{
        "description": "A summarization prompt",
        "messages": [
            {
                "role": "user",
                "content": { "type": "text", "text": "Please summarize this." }
            },
            {
                "role": "assistant",
                "content": { "type": "text", "text": "Here is a summary." }
            }
        ]
    }"#).unwrap();

    let pm = PromptMessages::from_json(&json).unwrap();
    assert_eq!(pm.description.as_deref(), Some("A summarization prompt"));
    assert_eq!(pm.messages.len(), 2);
    assert_eq!(pm.messages[0].role, "user");
    match &pm.messages[0].content {
        ContentItem::Text(t) => assert_eq!(t, "Please summarize this."),
        _ => panic!("expected Text content"),
    }
    assert_eq!(pm.messages[1].role, "assistant");
}

#[test]
fn parse_prompt_messages_no_description() {
    let json = JsonValue::parse(r#"{
        "messages": [
            {
                "role": "user",
                "content": { "type": "text", "text": "Hi" }
            }
        ]
    }"#).unwrap();

    let pm = PromptMessages::from_json(&json).unwrap();
    assert!(pm.description.is_none());
    assert_eq!(pm.messages.len(), 1);
}
