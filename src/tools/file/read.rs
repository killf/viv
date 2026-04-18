use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Reads a file from the local filesystem. You can access any file directly by using this tool.\n\nAssume this tool is able to read all files on the machine.\n\n- The file_path parameter must be an absolute path, not a relative path\n- By default, it reads up to 2000 lines starting from the beginning of the file\n- You can optionally specify a line offset and limit (especially handy for long files)\n- Results are returned using cat -n format, with line numbers starting at 1\n- For PDF files, provide the pages parameter to read specific page ranges (e.g., \"1-5\"). Maximum 20 pages per request."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"The absolute path to the file to read"},
                "offset":{"type":"number","description":"The line number to start reading from. Only provide if the file is too large to read at once (1-based)."},
                "limit":{"type":"number","description":"The number of lines to read. Only provide if the file is too large to read at once."},
                "pages":{"type":"string","description":"Page range for PDF files (e.g., \"1-5\", \"3\", \"10-20\"). Only applicable to PDF files. Maximum 20 pages per request."}
            },
            "required":["file_path"]
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

            let content = std::fs::read_to_string(path)
                .map_err(|e| Error::Tool(format!("cannot read '{}': {}", path, e)))?;

            let offset = input
                .get("offset")
                .and_then(|v| v.as_i64())
                .unwrap_or(1)
                .max(1) as usize;
            let limit = input
                .get("limit")
                .and_then(|v| v.as_i64())
                .map(|n| n as usize)
                .unwrap_or(2000);

            let lines: Vec<&str> = content.lines().collect();
            let start = (offset - 1).min(lines.len());
            let end = (start + limit).min(lines.len());

            let mut out = String::new();
            for (i, line) in lines[start..end].iter().enumerate() {
                out.push_str(&format!("{}\t{}\n", start + i + 1, line));
            }
            Ok(out)
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
