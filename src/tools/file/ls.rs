use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct LsTool;

impl Tool for LsTool {
    fn name(&self) -> &str {
        "LS"
    }

    fn description(&self) -> &str {
        "Lists files and directories in a given path. Directories are shown with a trailing slash. Entries are sorted alphabetically.\n\nPrefer Glob for pattern-based file discovery. Use LS when you want to see the full contents of a specific directory."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "path":{"type":"string","description":"The directory path to list. Defaults to the current working directory."}
            }
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
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
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
