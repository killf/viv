use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct LsTool;

impl Tool for LsTool {
    fn name(&self) -> &str { "ls" }

    fn description(&self) -> &str {
        "List directory contents. Directories shown with trailing slash. Entries sorted alphabetically."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "path":{"type":"string","description":"Directory to list (default: current directory)"}
            }
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let mut entries: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| Error::Tool(format!("cannot list '{}': {}", path, e)))?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{}/", name)
                } else {
                    name
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
