use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str { "edit" }

    fn description(&self) -> &str {
        "Replace an exact string in a file. Fails if old_string is not found or appears more than once (unless replace_all: true)."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string"},
                "old_string":{"type":"string","description":"Exact text to replace"},
                "new_string":{"type":"string","description":"Replacement text"},
                "replace_all":{"type":"boolean","description":"Replace all occurrences (default: false)"}
            },
            "required":["file_path","old_string","new_string"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let old = input.get("old_string").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'old_string'".into()))?;
        let new = input.get("new_string").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'new_string'".into()))?;
        let replace_all = input.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

        let count = content.matches(old).count();
        if count == 0 {
            return Err(Error::Tool(format!("'old_string' not found in '{}'", path)));
        }
        if count > 1 && !replace_all {
            return Err(Error::Tool(format!(
                "'old_string' appears {} times in '{}'; use replace_all: true", count, path
            )));
        }

        let new_content = if replace_all { content.replace(old, new) } else { content.replacen(old, new, 1) };
        std::fs::write(path, &new_content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Replaced {} occurrence(s) in {}", if replace_all { count } else { 1 }, path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    fn name(&self) -> &str { "multi_edit" }

    fn description(&self) -> &str {
        "Apply multiple edits to one file in order. Each edit is {old_string, new_string, replace_all?}. All edits succeed or none are written."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string"},
                "edits":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{
                            "old_string":{"type":"string"},
                            "new_string":{"type":"string"},
                            "replace_all":{"type":"boolean"}
                        },
                        "required":["old_string","new_string"]
                    }
                }
            },
            "required":["file_path","edits"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let edits = input.get("edits").and_then(|v| v.as_array())
            .ok_or_else(|| Error::Tool("missing 'edits'".into()))?;

        let mut content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

        for (i, edit) in edits.iter().enumerate() {
            let old = edit.get("old_string").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'old_string'", i)))?;
            let new = edit.get("new_string").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'new_string'", i)))?;
            let replace_all = edit.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

            let count = content.matches(old).count();
            if count == 0 {
                return Err(Error::Tool(format!("edits[{}]: 'old_string' not found", i)));
            }
            if count > 1 && !replace_all {
                return Err(Error::Tool(format!(
                    "edits[{}]: 'old_string' appears {} times; use replace_all: true", i, count
                )));
            }
            content = if replace_all { content.replace(old, new) } else { content.replacen(old, new, 1) };
        }

        std::fs::write(path, &content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Applied {} edit(s) to {}", edits.len(), path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}
