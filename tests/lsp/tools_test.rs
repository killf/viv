use std::sync::{Arc, Mutex};
use viv::core::json::JsonValue;
use viv::lsp::LspManager;
use viv::lsp::config::LspConfig;
use viv::lsp::tools::{LspDefinitionTool, LspDiagnosticsTool, LspHoverTool, LspReferencesTool};
use viv::tools::{PermissionLevel, Tool};

fn empty_manager() -> Arc<Mutex<LspManager>> {
    let config = LspConfig::parse("{}").unwrap();
    Arc::new(Mutex::new(LspManager::new(config)))
}

// ---------------------------------------------------------------------------
// LspDefinitionTool metadata
// ---------------------------------------------------------------------------

#[test]
fn definition_tool_metadata() {
    let mgr = empty_manager();
    let tool = LspDefinitionTool::new(mgr);

    assert_eq!(tool.name(), "LspDefinition");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);

    let schema = tool.input_schema();
    // Schema should have properties with file, line, and column
    let props = schema.get("properties").expect("schema has properties");
    assert!(props.get("file").is_some(), "schema has 'file' property");
    assert!(props.get("line").is_some(), "schema has 'line' property");
    assert!(
        props.get("column").is_some(),
        "schema has 'column' property"
    );

    // All three required
    let required = schema.get("required").expect("schema has required");
    let req_str = format!("{}", required);
    assert!(req_str.contains("file"));
    assert!(req_str.contains("line"));
    assert!(req_str.contains("column"));
}

// ---------------------------------------------------------------------------
// LspReferencesTool metadata
// ---------------------------------------------------------------------------

#[test]
fn references_tool_metadata() {
    let mgr = empty_manager();
    let tool = LspReferencesTool::new(mgr);

    assert_eq!(tool.name(), "LspReferences");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);
}

// ---------------------------------------------------------------------------
// LspHoverTool metadata
// ---------------------------------------------------------------------------

#[test]
fn hover_tool_metadata() {
    let mgr = empty_manager();
    let tool = LspHoverTool::new(mgr);

    assert_eq!(tool.name(), "LspHover");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);
}

// ---------------------------------------------------------------------------
// LspDiagnosticsTool metadata
// ---------------------------------------------------------------------------

#[test]
fn diagnostics_tool_metadata() {
    let mgr = empty_manager();
    let tool = LspDiagnosticsTool::new(mgr);

    assert_eq!(tool.name(), "LspDiagnostics");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);

    let schema = tool.input_schema();
    // Schema should have a 'file' property
    let props = schema.get("properties").expect("schema has properties");
    assert!(props.get("file").is_some(), "schema has 'file' property");

    // No required fields — the 'file' parameter is optional
    assert!(
        schema.get("required").is_none(),
        "diagnostics schema should have no required fields"
    );
}

// ---------------------------------------------------------------------------
// Description sanity checks
// ---------------------------------------------------------------------------

#[test]
fn all_tools_have_non_empty_descriptions() {
    let mgr = empty_manager();

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(LspDefinitionTool::new(Arc::clone(&mgr))),
        Box::new(LspReferencesTool::new(Arc::clone(&mgr))),
        Box::new(LspHoverTool::new(Arc::clone(&mgr))),
        Box::new(LspDiagnosticsTool::new(Arc::clone(&mgr))),
    ];

    for tool in &tools {
        assert!(
            !tool.description().is_empty(),
            "tool '{}' has empty description",
            tool.name()
        );
    }
}

// ---------------------------------------------------------------------------
// Execute with no file returns error (no LSP server configured)
// ---------------------------------------------------------------------------

#[test]
fn definition_tool_no_server_returns_error() {
    let mgr = empty_manager();
    let tool = LspDefinitionTool::new(mgr);

    let input = JsonValue::Object(vec![
        (
            "file".to_string(),
            JsonValue::Str("/tmp/test.rs".to_string()),
        ),
        (
            "line".to_string(),
            JsonValue::Number(viv::core::json::Number::Int(0)),
        ),
        (
            "column".to_string(),
            JsonValue::Number(viv::core::json::Number::Int(0)),
        ),
    ]);

    let result = viv::tools::poll_to_completion(tool.execute(&input));
    // Should fail because no LSP server is configured
    assert!(
        result.is_err(),
        "expected error when no LSP server configured"
    );
}
