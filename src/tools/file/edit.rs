use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Performs exact string replacements in files.\n\nUsage:\n- You must use your Read tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.\n- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix.\n- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.\n- Use `replace_all` for replacing and renaming strings across the file."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"The absolute path to the file to modify"},
                "old_string":{"type":"string","description":"The text to replace"},
                "new_string":{"type":"string","description":"The text to replace it with (must be different from old_string)"},
                "replace_all":{"type":"boolean","description":"Replace all occurrences of old_string (default false)"}
            },
            "required":["file_path","old_string","new_string"]
        }"#)
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
            let old = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'old_string'".into()))?;
            let new = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'new_string'".into()))?;
            let replace_all = input
                .get("replace_all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let content = std::fs::read_to_string(path)
                .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

            let count = content.matches(old).count();
            if count == 0 {
                return Err(Error::Tool(format!("'old_string' not found in '{}'", path)));
            }
            if count > 1 && !replace_all {
                return Err(Error::Tool(format!(
                    "'old_string' appears {} times in '{}'; use replace_all: true",
                    count, path
                )));
            }

            let new_content = if replace_all {
                content.replace(old, new)
            } else {
                content.replacen(old, new, 1)
            };
            std::fs::write(path, &new_content)
                .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
            Ok(format!(
                "Replaced {} occurrence(s) in {}",
                if replace_all { count } else { 1 },
                path
            ))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
}

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }

    fn description(&self) -> &str {
        "Performs multiple exact string replacements in a single file atomically.\n\nAll edits are applied in sequence. If any edit fails (old_string not found or not unique), none of the edits are written — the file remains unchanged.\n\nEach edit must have a unique `old_string` in the current state of the file (after prior edits in the sequence). Prefer this over multiple Edit calls when changing the same file."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"The absolute path to the file to modify"},
                "edits":{
                    "type":"array",
                    "description":"Array of edits to apply in sequence",
                    "items":{
                        "type":"object",
                        "properties":{
                            "old_string":{"type":"string","description":"The text to replace"},
                            "new_string":{"type":"string","description":"The text to replace it with"},
                            "replace_all":{"type":"boolean","description":"Replace all occurrences (default false)"}
                        },
                        "required":["old_string","new_string"]
                    }
                }
            },
            "required":["file_path","edits"]
        }"#)
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
            let edits = input
                .get("edits")
                .and_then(|v| v.as_array())
                .ok_or_else(|| Error::Tool("missing 'edits'".into()))?;

            let mut content = std::fs::read_to_string(path)
                .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

            for (i, edit) in edits.iter().enumerate() {
                let old = edit
                    .get("old_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'old_string'", i)))?;
                let new = edit
                    .get("new_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'new_string'", i)))?;
                let replace_all = edit
                    .get("replace_all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let count = content.matches(old).count();
                if count == 0 {
                    return Err(Error::Tool(format!("edits[{}]: 'old_string' not found", i)));
                }
                if count > 1 && !replace_all {
                    return Err(Error::Tool(format!(
                        "edits[{}]: 'old_string' appears {} times; use replace_all: true",
                        i, count
                    )));
                }
                content = if replace_all {
                    content.replace(old, new)
                } else {
                    content.replacen(old, new, 1)
                };
            }

            std::fs::write(path, &content)
                .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
            Ok(format!("Applied {} edit(s) to {}", edits.len(), path))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
}
