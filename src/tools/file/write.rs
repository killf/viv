use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct WriteTool;

impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Writes a file to the local filesystem.\n\n- This tool will overwrite the existing file if there is one at the provided path\n- If this is an existing file, you MUST use the Read tool first to read the file's contents\n- NEVER create documentation files (*.md) or README files unless explicitly requested\n- Prefer the Edit tool for modifying existing files — only use this tool to create new files or for complete rewrites"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"The absolute path to the file to write (must be absolute, not relative)"},
                "content":{"type":"string","description":"The content to write to the file"}
            },
            "required":["file_path","content"]
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'content'".into()))?;

            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
            }
            std::fs::write(path, content)
                .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
            Ok(format!("Wrote {} bytes to {}", content.len(), path))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
}
