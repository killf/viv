use crate::Error;
use crate::core::json::JsonValue;

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let line = json
            .get("line")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::Json("Position missing 'line'".to_string()))?;

        let character = json
            .get("character")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::Json("Position missing 'character'".to_string()))?;

        Ok(Position {
            line: line as u32,
            character: character as u32,
        })
    }

    pub fn to_json(&self) -> String {
        format!(r#"{{"line":{},"character":{}}}"#, self.line, self.character)
    }
}

// ---------------------------------------------------------------------------
// Range
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let start = json
            .get("start")
            .ok_or_else(|| Error::Json("Range missing 'start'".to_string()))
            .and_then(Position::from_json)?;

        let end = json
            .get("end")
            .ok_or_else(|| Error::Json("Range missing 'end'".to_string()))
            .and_then(Position::from_json)?;

        Ok(Range { start, end })
    }
}

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("Location missing 'uri'".to_string()))?
            .to_string();

        let range = json
            .get("range")
            .ok_or_else(|| Error::Json("Location missing 'range'".to_string()))
            .and_then(Range::from_json)?;

        Ok(Location { uri, range })
    }

    /// Returns the filesystem path by stripping the `file://` prefix if present.
    pub fn file_path(&self) -> &str {
        self.uri.strip_prefix("file://").unwrap_or(&self.uri)
    }
}

// ---------------------------------------------------------------------------
// HoverResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverResult {
    pub contents: String,
}

impl HoverResult {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let contents_val = json
            .get("contents")
            .ok_or_else(|| Error::Json("HoverResult missing 'contents'".to_string()))?;

        let contents = Self::extract_contents(contents_val)?;
        Ok(HoverResult { contents })
    }

    fn extract_contents(val: &JsonValue) -> crate::Result<String> {
        match val {
            // Plain string
            JsonValue::Str(s) => Ok(s.clone()),

            // MarkupContent object: { kind, value }
            JsonValue::Object(_) => {
                if let Some(value) = val.get("value").and_then(|v| v.as_str()) {
                    Ok(value.to_string())
                } else {
                    Err(Error::Json(
                        "HoverResult MarkupContent missing 'value'".to_string(),
                    ))
                }
            }

            // Array of MarkedString (string | { language, value })
            JsonValue::Array(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|item| match item {
                        JsonValue::Str(s) => s.clone(),
                        JsonValue::Object(_) => item
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        _ => String::new(),
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                Ok(parts.join("\n"))
            }

            _ => Err(Error::Json(
                "HoverResult 'contents' has unexpected type".to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticSeverity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    pub fn from_i64(n: i64) -> Option<Self> {
        match n {
            1 => Some(DiagnosticSeverity::Error),
            2 => Some(DiagnosticSeverity::Warning),
            3 => Some(DiagnosticSeverity::Information),
            4 => Some(DiagnosticSeverity::Hint),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let range = json
            .get("range")
            .ok_or_else(|| Error::Json("Diagnostic missing 'range'".to_string()))
            .and_then(Range::from_json)?;

        let severity = json
            .get("severity")
            .and_then(|v| v.as_i64())
            .and_then(DiagnosticSeverity::from_i64);

        let message = json
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("Diagnostic missing 'message'".to_string()))?
            .to_string();

        let code = json
            .get("code")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Diagnostic {
            range,
            severity,
            message,
            code,
        })
    }
}

// ---------------------------------------------------------------------------
// LspServerCapabilities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LspServerCapabilities {
    pub definition: bool,
    pub references: bool,
    pub hover: bool,
}

impl LspServerCapabilities {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let caps = json.get("capabilities").ok_or_else(|| {
            Error::Json("LspServerCapabilities missing 'capabilities'".to_string())
        })?;

        let definition = caps
            .get("definitionProvider")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let references = caps
            .get("referencesProvider")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let hover = caps
            .get("hoverProvider")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(LspServerCapabilities {
            definition,
            references,
            hover,
        })
    }
}
