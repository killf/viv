use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }

    fn description(&self) -> &str {
        "Read a file and return its contents with line numbers. Use offset (1-based line) and limit to read a specific range."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"Path to the file"},
                "offset":{"type":"number","description":"Line to start reading from (1-based, default 1)"},
                "limit":{"type":"number","description":"Maximum number of lines to return"}
            },
            "required":["file_path"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("cannot read '{}': {}", path, e)))?;

        let offset = input.get("offset").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        let limit = input.get("limit").and_then(|v| v.as_i64()).map(|n| n as usize);

        let lines: Vec<&str> = content.lines().collect();
        let start = (offset - 1).min(lines.len());
        let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());

        let mut out = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            out.push_str(&format!("{}\t{}\n", start + i + 1, line));
        }
        Ok(out)
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
