use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target"];

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching tool that works with any codebase size.\n\n- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n- Returns matching file paths sorted by modification time (most recent first)\n- Automatically ignores .git/, node_modules/, and target/ directories\n- Use this tool when you need to find files by name patterns"
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(r#"{
            "type":"object",
            "properties":{
                "pattern":{"type":"string","description":"The glob pattern to match files against"},
                "path":{"type":"string","description":"The directory to search in. If not specified, the current working directory will be used."}
            },
            "required":["pattern"]
        }"#)
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
            let root = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

            let mut matches: Vec<PathBuf> = vec![];
            let parts: Vec<&str> = pattern.split('/').collect();
            walk_glob(Path::new(root), &parts, &mut matches)
                .map_err(|e| Error::Tool(format!("glob walk: {}", e)))?;
            matches.sort_by(|a, b| {
                let ma = a
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let mb = b
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                mb.cmp(&ma)
            });
            Ok(matches
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

fn walk_glob(dir: &Path, parts: &[&str], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if parts.is_empty() {
        return Ok(());
    }
    let seg = parts[0];
    let rest = &parts[1..];

    if seg == "**" {
        if !rest.is_empty() {
            walk_glob(dir, rest, out)?;
        } else {
            collect_all(dir, out)?;
            return Ok(());
        }
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)?.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if IGNORED_DIRS.contains(&name) {
                            continue;
                        }
                    }
                    walk_glob(&p, parts, out)?;
                }
            }
        }
        return Ok(());
    }

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if IGNORED_DIRS.contains(&name) {
                        continue;
                    }
                }
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if segment_match(seg, name) {
                if rest.is_empty() {
                    out.push(p);
                } else if p.is_dir() {
                    walk_glob(&p, rest, out)?;
                }
            }
        }
    }
    Ok(())
}

fn collect_all(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if IGNORED_DIRS.contains(&name) {
                        continue;
                    }
                }
            }
            out.push(p.clone());
            if p.is_dir() {
                collect_all(&p, out)?;
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
    for i in 1..=m {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if p[i - 1] == '*' {
                dp[i - 1][j] || dp[i][j - 1]
            } else if p[i - 1] == '?' || p[i - 1] == t[j - 1] {
                dp[i - 1][j - 1]
            } else {
                false
            };
        }
    }
    dp[m][n]
}
