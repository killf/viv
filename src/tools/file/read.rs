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
        "Reads a file from the local filesystem.\n\n- The file_path parameter must be an absolute path, not a relative path\n- By default, it reads up to 2000 lines starting from the beginning of the file\n- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters\n- Results are returned using cat -n format, with line numbers starting at 1\n- This tool can read PDF files (.pdf) when the pages parameter is provided\n- For binary files, a message is returned instead of raw content\n- If you read a file that exists but has empty contents you will receive a warning"
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"The absolute path to the file to read"},
                "offset":{"type":"number","description":"The line number to start reading from. Only provide if the file is too large to read at once (1-based)."},
                "limit":{"type":"number","description":"The number of lines to read. Only provide if the file is too large to read at once."},
                "pages":{"type":"string","description":"Page range for PDF files (e.g., \"1-5\", \"3\", \"10-20\"). Only applicable to PDF files. Maximum 20 pages per request."}
            },
            "required":["file_path"]
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

            if path.ends_with(".pdf") {
                let pages = input.get("pages").and_then(|v| v.as_str());
                return read_pdf(path, pages);
            }

            let raw = std::fs::read(path)
                .map_err(|e| Error::Tool(format!("cannot read '{}': {}", path, e)))?;
            if raw.iter().take(512).any(|&b| b == 0) {
                return Ok(format!(
                    "Binary file '{}' — cannot display as text ({} bytes)",
                    path,
                    raw.len()
                ));
            }

            let content = String::from_utf8_lossy(&raw);
            if content.is_empty() {
                return Ok(format!("File '{}' exists but is empty", path));
            }

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

fn read_pdf(path: &str, pages: Option<&str>) -> crate::Result<String> {
    use std::process::Command;
    let mut cmd = Command::new("pdftotext");
    if let Some(range) = pages {
        let parts: Vec<&str> = range.split('-').collect();
        if let Some(first) = parts.first() {
            cmd.arg("-f").arg(first);
        }
        if let Some(last) = parts.get(1) {
            cmd.arg("-l").arg(last);
        }
    }
    cmd.arg(path).arg("-");
    let output = cmd.output().map_err(|e| {
        Error::Tool(format!(
            "pdftotext not found or failed: {}. Install poppler-utils for PDF support.",
            e
        ))
    })?;
    if !output.status.success() {
        return Err(Error::Tool(format!(
            "pdftotext error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
