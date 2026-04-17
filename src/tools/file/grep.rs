use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};
use std::path::{Path, PathBuf};

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }

    fn description(&self) -> &str {
        "Search for a literal pattern in files. output_mode: 'files_with_matches' (default), 'content' (show matching lines with line numbers), 'count'."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "pattern":{"type":"string","description":"Literal string to search for"},
                "path":{"type":"string","description":"File or directory to search (default: current directory)"},
                "glob":{"type":"string","description":"Filename glob filter, e.g. \"*.rs\""},
                "output_mode":{"type":"string","description":"files_with_matches | content | count"}
            },
            "required":["pattern"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let pattern = input.get("pattern").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'pattern'".into()))?;
        let root = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let glob_filter = input.get("glob").and_then(|v| v.as_str());
        let mode = input.get("output_mode").and_then(|v| v.as_str()).unwrap_or("files_with_matches");

        let mut files: Vec<PathBuf> = vec![];
        collect_files(Path::new(root), glob_filter, &mut files)
            .map_err(|e| Error::Tool(format!("walk: {}", e)))?;
        files.sort();

        let mut out = String::new();
        for file in &files {
            let content = match std::fs::read_to_string(file) { Ok(c) => c, Err(_) => continue };
            match mode {
                "content" => {
                    for (i, line) in content.lines().enumerate() {
                        if line.contains(pattern) {
                            out.push_str(&format!("{}:{}:{}\n", file.display(), i + 1, line));
                        }
                    }
                }
                "count" => {
                    let c = content.lines().filter(|l| l.contains(pattern)).count();
                    if c > 0 { out.push_str(&format!("{}:{}\n", file.display(), c)); }
                }
                _ => {
                    if content.contains(pattern) { out.push_str(&format!("{}\n", file.display())); }
                }
            }
        }
        Ok(out.trim_end().to_string())
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

fn collect_files(dir: &Path, filter: Option<&str>, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_files(&p, filter, out)?;
            } else {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if filter.map(|f| segment_match(f, name)).unwrap_or(true) {
                    out.push(p);
                }
            }
        }
    }
    Ok(())
}

fn segment_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (m, n) = (p.len(), t.len());
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for i in 1..=m { if p[i-1] == '*' { dp[i][0] = dp[i-1][0]; } }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if p[i-1] == '*' { dp[i-1][j] || dp[i][j-1] }
                       else if p[i-1] == '?' || p[i-1] == t[j-1] { dp[i-1][j-1] }
                       else { false };
        }
    }
    dp[m][n]
}
