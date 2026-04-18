use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "A powerful search tool that supports full regex syntax (e.g., \"log.*Error\", \"function\\s+\\w+\").\n\n- Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\")\n- Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts\n- Use -A/-B/-C for context lines around matches\n- Pattern syntax uses extended regex (ERE)"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "pattern":{"type":"string","description":"The regular expression pattern to search for in file contents"},
                "path":{"type":"string","description":"File or directory to search in. Defaults to current working directory."},
                "glob":{"type":"string","description":"Glob pattern to filter files (e.g. \"*.js\", \"*.{ts,tsx}\")"},
                "type":{"type":"string","description":"File type to search (e.g. \"js\", \"py\", \"rs\", \"go\"). Expands to --include=\"*.{type}\""},
                "output_mode":{"type":"string","description":"Output mode: \"content\" shows matching lines (with -A/-B/-C context), \"files_with_matches\" shows file paths (default), \"count\" shows match counts"},
                "-i":{"type":"boolean","description":"Case insensitive search"},
                "-n":{"type":"boolean","description":"Show line numbers in output. Requires output_mode: \"content\". Defaults to true."},
                "-A":{"type":"number","description":"Number of lines to show after each match. Requires output_mode: \"content\"."},
                "-B":{"type":"number","description":"Number of lines to show before each match. Requires output_mode: \"content\"."},
                "-C":{"type":"number","description":"Number of lines to show before and after each match. Requires output_mode: \"content\"."},
                "head_limit":{"type":"number","description":"Limit output to first N lines/entries. Defaults to 250 when unspecified. Pass 0 for unlimited."},
                "offset":{"type":"number","description":"Skip first N lines/entries before applying head_limit. Defaults to 0."},
                "multiline":{"type":"boolean","description":"Enable multiline mode where . matches newlines (uses -z flag). Default: false."}
            },
            "required":["pattern"]
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'pattern'".into()))?;
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let glob_filter = input.get("glob").and_then(|v| v.as_str());
            let type_filter = input.get("type").and_then(|v| v.as_str());
            let mode = input
                .get("output_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("files_with_matches");
            let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
            let show_line_numbers = input.get("-n").and_then(|v| v.as_bool()).unwrap_or(true);
            let lines_after = input.get("-A").and_then(|v| v.as_i64()).unwrap_or(0);
            let lines_before = input.get("-B").and_then(|v| v.as_i64()).unwrap_or(0);
            let context = input.get("-C").and_then(|v| v.as_i64()).unwrap_or(0);
            let head_limit = input
                .get("head_limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(250) as usize;
            let offset = input.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
            let multiline = input
                .get("multiline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let mut cmd = Command::new("grep");
            cmd.arg("-E"); // extended regex

            match mode {
                "files_with_matches" => {
                    cmd.arg("-l");
                }
                "count" => {
                    cmd.arg("-c");
                }
                "content" => {
                    if show_line_numbers {
                        cmd.arg("-n");
                    }
                }
                _ => {
                    cmd.arg("-l");
                }
            }

            if case_insensitive {
                cmd.arg("-i");
            }
            if multiline {
                cmd.arg("-z");
            }

            let a = lines_after.max(context);
            let b = lines_before.max(context);
            if a > 0 {
                cmd.arg(format!("-A{}", a));
            }
            if b > 0 {
                cmd.arg(format!("-B{}", b));
            }

            // Include filter: type takes precedence over glob
            if let Some(t) = type_filter {
                cmd.arg(format!("--include=*.{}", t));
            } else if let Some(g) = glob_filter {
                cmd.arg(format!("--include={}", g));
            }

            cmd.arg("-r").arg("--").arg(pattern).arg(path);

            let output = cmd
                .output()
                .map_err(|e| Error::Tool(format!("grep failed: {}", e)))?;

            // grep exits with 1 when no matches (not an error), 2 on real errors
            if output.status.code() == Some(2) {
                let err = String::from_utf8_lossy(&output.stderr).into_owned();
                return Err(Error::Tool(format!("grep error: {}", err.trim())));
            }

            let raw = String::from_utf8_lossy(&output.stdout).into_owned();

            // For count mode, filter out zero-count lines
            let filtered: String = if mode == "count" {
                raw.lines()
                    .filter(|l| {
                        l.rsplit_once(':')
                            .and_then(|(_, n)| n.parse::<u64>().ok())
                            .map(|n| n > 0)
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                raw.trim_end().to_string()
            };

            // Apply offset and head_limit
            let lines: Vec<&str> = filtered.lines().collect();
            let start = offset.min(lines.len());
            let end = if head_limit == 0 {
                lines.len()
            } else {
                (start + head_limit).min(lines.len())
            };

            Ok(lines[start..end].join("\n"))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
