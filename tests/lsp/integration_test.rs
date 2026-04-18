use std::sync::{Arc, Mutex};
use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::lsp::LspManager;
use viv::lsp::config::LspConfig;
use viv::lsp::tools::{LspDefinitionTool, LspHoverTool, LspReferencesTool, LspDiagnosticsTool};
use viv::tools::Tool;

#[test]
fn definition_tool_errors_when_no_server_configured() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = Arc::new(Mutex::new(LspManager::new(config)));
    let tool = LspDefinitionTool::new(mgr);

    let input = JsonValue::Object(vec![
        ("file".into(), JsonValue::Str("src/main.rs".into())),
        ("line".into(), JsonValue::Number(Number::Int(0))),
        ("column".into(), JsonValue::Number(Number::Int(0))),
    ]);

    let result = block_on(async move { tool.execute(&input).await });
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("no LSP server configured") || err_msg.contains("LSP error"),
        "unexpected error: {}", err_msg);
}

#[test]
fn hover_tool_errors_when_no_server_configured() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = Arc::new(Mutex::new(LspManager::new(config)));
    let tool = LspHoverTool::new(mgr);

    let input = JsonValue::Object(vec![
        ("file".into(), JsonValue::Str("test.py".into())),
        ("line".into(), JsonValue::Number(Number::Int(5))),
        ("column".into(), JsonValue::Number(Number::Int(3))),
    ]);

    let result = block_on(async move { tool.execute(&input).await });
    assert!(result.is_err());
}

#[test]
fn references_tool_errors_when_no_server_configured() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = Arc::new(Mutex::new(LspManager::new(config)));
    let tool = LspReferencesTool::new(mgr);

    let input = JsonValue::Object(vec![
        ("file".into(), JsonValue::Str("app.js".into())),
        ("line".into(), JsonValue::Number(Number::Int(0))),
        ("column".into(), JsonValue::Number(Number::Int(0))),
    ]);

    let result = block_on(async move { tool.execute(&input).await });
    assert!(result.is_err());
}

#[test]
fn diagnostics_tool_returns_empty_when_no_servers_running() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = Arc::new(Mutex::new(LspManager::new(config)));
    let tool = LspDiagnosticsTool::new(mgr);

    // Diagnostics with no file param should return "No diagnostics." when no servers running
    let input = JsonValue::Object(vec![]);
    let result = block_on(async move { tool.execute(&input).await });
    assert!(result.is_ok());
    assert!(result.unwrap().contains("No diagnostics"));
}
