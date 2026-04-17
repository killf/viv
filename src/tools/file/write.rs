use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct WriteTool;

impl Tool for WriteTool {
    fn name(&self) -> &str { "write" }

    fn description(&self) -> &str {
        "Write content to a file, overwriting it if it already exists. Creates parent directories automatically."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"Path to the file to write"},
                "content":{"type":"string","description":"Content to write"}
            },
            "required":["file_path","content"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let content = input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'content'".into()))?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
        }
        std::fs::write(path, content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Wrote {} bytes to {}", content.len(), path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}
