use viv::core::json::JsonValue;
use viv::lsp::types::*;

// --- Position ---

#[test]
fn parse_position() {
    let json = JsonValue::parse(r#"{"line": 10, "character": 5}"#).unwrap();
    let pos = Position::from_json(&json).unwrap();
    assert_eq!(pos.line, 10);
    assert_eq!(pos.character, 5);
}

#[test]
fn position_to_json() {
    let pos = Position {
        line: 3,
        character: 7,
    };
    let json_str = pos.to_json();
    let parsed = JsonValue::parse(&json_str).unwrap();
    assert_eq!(parsed.get("line").and_then(|v| v.as_i64()), Some(3));
    assert_eq!(parsed.get("character").and_then(|v| v.as_i64()), Some(7));
}

// --- Location ---

#[test]
fn parse_location() {
    let json = JsonValue::parse(
        r#"{
        "uri": "file:///home/user/main.rs",
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 0, "character": 10}
        }
    }"#,
    )
    .unwrap();
    let loc = Location::from_json(&json).unwrap();
    assert_eq!(loc.uri, "file:///home/user/main.rs");
    assert_eq!(loc.range.start.line, 0);
    assert_eq!(loc.range.start.character, 0);
    assert_eq!(loc.range.end.line, 0);
    assert_eq!(loc.range.end.character, 10);
}

#[test]
fn location_file_path_strips_prefix() {
    let json = JsonValue::parse(
        r#"{
        "uri": "file:///home/user/project/src/main.rs",
        "range": {
            "start": {"line": 1, "character": 0},
            "end": {"line": 1, "character": 5}
        }
    }"#,
    )
    .unwrap();
    let loc = Location::from_json(&json).unwrap();
    assert_eq!(loc.file_path(), "/home/user/project/src/main.rs");
}

#[test]
fn location_file_path_no_prefix() {
    let json = JsonValue::parse(
        r#"{
        "uri": "/absolute/path/file.rs",
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 0, "character": 0}
        }
    }"#,
    )
    .unwrap();
    let loc = Location::from_json(&json).unwrap();
    assert_eq!(loc.file_path(), "/absolute/path/file.rs");
}

// --- HoverResult ---

#[test]
fn parse_hover_result_with_string() {
    let json = JsonValue::parse(r#"{"contents": "plain hover text"}"#).unwrap();
    let hover = HoverResult::from_json(&json).unwrap();
    assert_eq!(hover.contents, "plain hover text");
}

#[test]
fn parse_hover_result_with_markup() {
    let json = JsonValue::parse(
        r#"{
        "contents": {
            "kind": "markdown",
            "value": "**bold** hover text"
        }
    }"#,
    )
    .unwrap();
    let hover = HoverResult::from_json(&json).unwrap();
    assert_eq!(hover.contents, "**bold** hover text");
}

#[test]
fn parse_hover_result_with_array() {
    let json = JsonValue::parse(
        r#"{
        "contents": [
            {"language": "rust", "value": "fn foo()"},
            "some docs"
        ]
    }"#,
    )
    .unwrap();
    let hover = HoverResult::from_json(&json).unwrap();
    // Array elements joined; at minimum not empty
    assert!(!hover.contents.is_empty());
}

// --- DiagnosticSeverity ---

#[test]
fn diagnostic_severity_from_i64() {
    assert_eq!(
        DiagnosticSeverity::from_i64(1),
        Some(DiagnosticSeverity::Error)
    );
    assert_eq!(
        DiagnosticSeverity::from_i64(2),
        Some(DiagnosticSeverity::Warning)
    );
    assert_eq!(
        DiagnosticSeverity::from_i64(3),
        Some(DiagnosticSeverity::Information)
    );
    assert_eq!(
        DiagnosticSeverity::from_i64(4),
        Some(DiagnosticSeverity::Hint)
    );
    assert_eq!(DiagnosticSeverity::from_i64(5), None);
}

#[test]
fn diagnostic_severity_labels() {
    assert_eq!(DiagnosticSeverity::Error.label(), "error");
    assert_eq!(DiagnosticSeverity::Warning.label(), "warning");
    assert_eq!(DiagnosticSeverity::Information.label(), "info");
    assert_eq!(DiagnosticSeverity::Hint.label(), "hint");
}

// --- Diagnostic ---

#[test]
fn parse_diagnostic() {
    let json = JsonValue::parse(
        r#"{
        "range": {
            "start": {"line": 5, "character": 2},
            "end": {"line": 5, "character": 8}
        },
        "severity": 1,
        "message": "undefined variable",
        "code": "E0425"
    }"#,
    )
    .unwrap();
    let diag = Diagnostic::from_json(&json).unwrap();
    assert_eq!(diag.range.start.line, 5);
    assert_eq!(diag.range.start.character, 2);
    assert_eq!(diag.severity, Some(DiagnosticSeverity::Error));
    assert_eq!(diag.message, "undefined variable");
    assert_eq!(diag.code.as_deref(), Some("E0425"));
}

#[test]
fn parse_diagnostic_minimal() {
    let json = JsonValue::parse(
        r#"{
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 0, "character": 1}
        },
        "message": "warning: unused import"
    }"#,
    )
    .unwrap();
    let diag = Diagnostic::from_json(&json).unwrap();
    assert!(diag.severity.is_none());
    assert!(diag.code.is_none());
    assert_eq!(diag.message, "warning: unused import");
}

// --- LspServerCapabilities ---

#[test]
fn parse_server_capabilities() {
    let json = JsonValue::parse(
        r#"{
        "capabilities": {
            "definitionProvider": true,
            "referencesProvider": false,
            "hoverProvider": true
        }
    }"#,
    )
    .unwrap();
    let caps = LspServerCapabilities::from_json(&json).unwrap();
    assert!(caps.definition);
    assert!(!caps.references);
    assert!(caps.hover);
}

#[test]
fn parse_server_capabilities_all_false() {
    let json = JsonValue::parse(
        r#"{
        "capabilities": {
            "definitionProvider": false,
            "referencesProvider": false,
            "hoverProvider": false
        }
    }"#,
    )
    .unwrap();
    let caps = LspServerCapabilities::from_json(&json).unwrap();
    assert!(!caps.definition);
    assert!(!caps.references);
    assert!(!caps.hover);
}

#[test]
fn parse_server_capabilities_missing_fields() {
    let json = JsonValue::parse(r#"{"capabilities": {}}"#).unwrap();
    let caps = LspServerCapabilities::from_json(&json).unwrap();
    assert!(!caps.definition);
    assert!(!caps.references);
    assert!(!caps.hover);
}
