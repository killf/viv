# Skill System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Claude Code-style Skill system: SKILL.md files in directories, two-level discovery, SkillTool for agent invocation, system prompt injection.

**Architecture:** SkillRegistry scans `~/.viv/skills/` and `.viv/skills/` at startup, parses SKILL.md frontmatter. SkillTool registered in ToolRegistry lets Agent invoke skills by name. Skill list injected into existing system prompt cache infrastructure.

**Tech Stack:** Rust (edition 2024), zero dependencies, existing Tool trait and prompt cache.

**Spec:** `docs/superpowers/specs/2026-04-18-skill-system-design.md`

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| `src/skill/mod.rs` | SkillEntry, SkillSource, SkillRegistry, parse_frontmatter, load |
| `src/skill/tool.rs` | SkillTool implementing Tool trait |
| `tests/skill/mod.rs` | Test module |
| `tests/skill/registry_test.rs` | Frontmatter parsing, loading, merge, lookup |
| `tests/skill/tool_test.rs` | SkillTool execute behavior |

### Modified Files

| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod skill;` |
| `src/agent/agent.rs:256` | Pass `skill_list` to `build_system_prompt` instead of `""` |
| `src/agent/agent.rs:98` | Add `skill_registry` field to Agent struct, init in constructor |

---

## Task 1: SkillRegistry + Frontmatter Parser

**Files:**
- Create: `src/skill/mod.rs`
- Create: `tests/skill/registry_test.rs`
- Create: `tests/skill/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/skill/registry_test.rs
use viv::skill::{parse_frontmatter, SkillEntry, SkillRegistry, SkillSource};

#[test]
fn parse_frontmatter_basic() {
    let content = "---\nname: commit\ndescription: Use when committing\n---\n\n# Commit\n\nDo stuff.";
    let (fields, body) = parse_frontmatter(content).unwrap();
    assert_eq!(fields.get("name").unwrap(), "commit");
    assert_eq!(fields.get("description").unwrap(), "Use when committing");
    assert!(body.contains("# Commit"));
}

#[test]
fn parse_frontmatter_no_frontmatter() {
    let content = "# Just Markdown\n\nNo frontmatter here.";
    assert!(parse_frontmatter(content).is_none());
}

#[test]
fn parse_frontmatter_empty_body() {
    let content = "---\nname: test\ndescription: desc\n---\n";
    let (fields, body) = parse_frontmatter(content).unwrap();
    assert_eq!(fields.get("name").unwrap(), "test");
    assert!(body.trim().is_empty());
}

#[test]
fn registry_empty() {
    let reg = SkillRegistry::new();
    assert!(reg.is_empty());
    assert!(reg.get("anything").is_none());
}

#[test]
fn registry_add_and_get() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".into(),
        description: "Use when committing".into(),
        content: "# Commit\nDo stuff.".into(),
        base_dir: "/tmp/skills/commit".into(),
        source: SkillSource::User,
    });
    assert!(!reg.is_empty());
    let entry = reg.get("commit").unwrap();
    assert_eq!(entry.name, "commit");
    assert_eq!(entry.source, SkillSource::User);
}

#[test]
fn registry_project_overrides_user() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "deploy".into(),
        description: "User version".into(),
        content: "user content".into(),
        base_dir: "/home/user/.viv/skills/deploy".into(),
        source: SkillSource::User,
    });
    reg.add(SkillEntry {
        name: "deploy".into(),
        description: "Project version".into(),
        content: "project content".into(),
        base_dir: "/project/.viv/skills/deploy".into(),
        source: SkillSource::Project,
    });
    let entry = reg.get("deploy").unwrap();
    assert_eq!(entry.description, "Project version");
    assert_eq!(entry.source, SkillSource::Project);
}

#[test]
fn format_for_prompt_empty() {
    let reg = SkillRegistry::new();
    assert!(reg.format_for_prompt().is_empty());
}

#[test]
fn format_for_prompt_lists_skills() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".into(),
        description: "Use when committing".into(),
        content: "...".into(),
        base_dir: "/tmp".into(),
        source: SkillSource::User,
    });
    let prompt = reg.format_for_prompt();
    assert!(prompt.contains("commit"));
    assert!(prompt.contains("Use when committing"));
}

#[test]
fn load_from_directory() {
    // Create temp skill directory
    let dir = "/tmp/viv_skill_test_load";
    let skill_dir = format!("{}/my-skill", dir);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        format!("{}/SKILL.md", skill_dir),
        "---\nname: my-skill\ndescription: Use when testing\n---\n\n# Test Skill\n\nContent here.",
    ).unwrap();

    let reg = SkillRegistry::load_from_dir(dir, SkillSource::User);
    let entry = reg.get("my-skill").unwrap();
    assert_eq!(entry.name, "my-skill");
    assert!(entry.content.contains("# Test Skill"));
    assert!(entry.base_dir.contains("my-skill"));

    // Cleanup
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn load_skips_dir_without_skill_md() {
    let dir = "/tmp/viv_skill_test_skip";
    let skill_dir = format!("{}/no-skill", dir);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(format!("{}/README.md", skill_dir), "not a skill").unwrap();

    let reg = SkillRegistry::load_from_dir(dir, SkillSource::User);
    assert!(reg.is_empty());

    std::fs::remove_dir_all(dir).ok();
}
```

```rust
// tests/skill/mod.rs
mod registry_test;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test skill 2>&1 | head -10`
Expected: module not found

- [ ] **Step 3: Implement SkillRegistry**

```rust
// src/skill/mod.rs
pub mod tool;

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    User,
    Project,
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub content: String,
    pub base_dir: String,
    pub source: SkillSource,
}

pub struct SkillRegistry {
    skills: Vec<SkillEntry>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// Load skills from user and project directories, project overrides user.
    pub fn load(user_dir: &str, project_dir: &str) -> Self {
        let mut reg = Self::load_from_dir(user_dir, SkillSource::User);
        let project = Self::load_from_dir(project_dir, SkillSource::Project);
        for entry in project.skills {
            reg.add(entry);
        }
        reg
    }

    /// Load all skills from a single directory.
    pub fn load_from_dir(dir: &str, source: SkillSource) -> Self {
        let mut reg = Self::new();
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => return reg, // directory doesn't exist, that's fine
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let skill_file = path.join("SKILL.md");
            if !skill_file.exists() { continue; }
            let content = match std::fs::read_to_string(&skill_file) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (fields, body) = match parse_frontmatter(&content) {
                Some(r) => r,
                None => continue,
            };
            let name = match fields.get("name") {
                Some(n) => n.clone(),
                None => continue,
            };
            let description = fields.get("description").cloned().unwrap_or_default();
            reg.add(SkillEntry {
                name,
                description,
                content: body,
                base_dir: path.to_string_lossy().into_owned(),
                source: source.clone(),
            });
        }
        reg
    }

    pub fn add(&mut self, entry: SkillEntry) {
        // Replace existing with same name
        self.skills.retain(|s| s.name != entry.name);
        self.skills.push(entry);
    }

    pub fn get(&self, name: &str) -> Option<&SkillEntry> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn list(&self) -> &[SkillEntry] {
        &self.skills
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Generate the skill list text for system prompt injection.
    pub fn format_for_prompt(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }
        let mut out = String::from("Available skills (invoke via Skill tool):\n");
        for skill in &self.skills {
            out.push_str(&format!("- {}: {}\n", skill.name, skill.description));
        }
        out
    }
}

/// Parse SKILL.md frontmatter (--- delimited key: value pairs).
/// Returns (fields, body_after_frontmatter) or None if no frontmatter.
pub fn parse_frontmatter(content: &str) -> Option<(HashMap<String, String>, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Find closing ---
    let after_first = &trimmed[3..].trim_start_matches(['\r', '\n']);
    let close = after_first.find("\n---")?;
    let frontmatter_str = &after_first[..close];
    let body_start = close + 4; // skip \n---
    let body = after_first[body_start..].trim_start_matches(['\r', '\n']);

    let mut fields = HashMap::new();
    for line in frontmatter_str.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            fields.insert(key, value);
        }
    }

    Some((fields, body.to_string()))
}
```

Add to `src/lib.rs`:
```rust
pub mod skill;
```

- [ ] **Step 4: Run tests**

Run: `cargo test skill -v`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/skill/ src/lib.rs tests/skill/
git commit -m "feat(skill): add SkillRegistry with frontmatter parsing and two-level loading"
```

---

## Task 2: SkillTool

**Files:**
- Create: `src/skill/tool.rs`
- Create: `tests/skill/tool_test.rs`
- Modify: `tests/skill/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/skill/tool_test.rs
use std::sync::Arc;
use viv::skill::{SkillEntry, SkillRegistry, SkillSource};
use viv::skill::tool::SkillTool;
use viv::core::json::JsonValue;

// Helper to create a registry with test skills
fn test_registry() -> Arc<SkillRegistry> {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".into(),
        description: "Use when committing".into(),
        content: "# Commit\n\nRun git commit.".into(),
        base_dir: "/home/user/.viv/skills/commit".into(),
        source: SkillSource::User,
    });
    reg.add(SkillEntry {
        name: "review".into(),
        description: "Use when reviewing code".into(),
        content: "# Review\n\nCheck code quality.".into(),
        base_dir: "/project/.viv/skills/review".into(),
        source: SkillSource::Project,
    });
    Arc::new(reg)
}

#[test]
fn skill_tool_name() {
    let tool = SkillTool::new(test_registry());
    assert_eq!(viv::tools::Tool::name(&tool), "Skill");
}

#[tokio::test]
async fn skill_tool_execute_found() {
    let tool = SkillTool::new(test_registry());
    let input = JsonValue::Object(vec![
        ("skill".into(), JsonValue::Str("commit".into())),
    ]);
    let result = viv::tools::Tool::execute(&tool, &input).await.unwrap();
    assert!(result.contains("Base directory for this skill:"));
    assert!(result.contains("# Commit"));
    assert!(result.contains("Run git commit."));
}

#[tokio::test]
async fn skill_tool_execute_not_found() {
    let tool = SkillTool::new(test_registry());
    let input = JsonValue::Object(vec![
        ("skill".into(), JsonValue::Str("nonexistent".into())),
    ]);
    let result = viv::tools::Tool::execute(&tool, &input).await.unwrap();
    assert!(result.contains("not found"));
    assert!(result.contains("commit")); // should list available skills
}

#[tokio::test]
async fn skill_tool_execute_with_args() {
    let tool = SkillTool::new(test_registry());
    let input = JsonValue::Object(vec![
        ("skill".into(), JsonValue::Str("commit".into())),
        ("args".into(), JsonValue::Str("-m 'fix'".into())),
    ]);
    let result = viv::tools::Tool::execute(&tool, &input).await.unwrap();
    assert!(result.contains("# Commit"));
}
```

Note: The tests use `#[tokio::test]` because Tool::execute returns a Future. Since viv is zero-dependency, check how existing tool tests handle async execution. If there's no tokio, use the project's own `block_on_local` or make tests synchronous by wrapping.

Actually, read the existing tool tests to see how they handle the async execute. Adapt accordingly.

- [ ] **Step 2: Implement SkillTool**

```rust
// src/skill/tool.rs
use std::sync::Arc;
use crate::core::json::JsonValue;
use crate::skill::SkillRegistry;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct SkillTool {
    registry: Arc<SkillRegistry>,
}

impl SkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Invoke a skill by name. Returns the skill's full content for you to follow."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".into(), JsonValue::Str("object".into())),
            ("properties".into(), JsonValue::Object(vec![
                ("skill".into(), JsonValue::Object(vec![
                    ("type".into(), JsonValue::Str("string".into())),
                    ("description".into(), JsonValue::Str("The skill name to invoke".into())),
                ])),
                ("args".into(), JsonValue::Object(vec![
                    ("type".into(), JsonValue::Str("string".into())),
                    ("description".into(), JsonValue::Str("Optional arguments for the skill".into())),
                ])),
            ])),
            ("required".into(), JsonValue::Array(vec![
                JsonValue::Str("skill".into()),
            ])),
        ])
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let skill_name = input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let registry = self.registry.clone();

        Box::pin(async move {
            match registry.get(&skill_name) {
                Some(entry) => {
                    Ok(format!(
                        "Base directory for this skill: {}\n\n{}",
                        entry.base_dir, entry.content
                    ))
                }
                None => {
                    let available: Vec<&str> = registry.list().iter().map(|s| s.name.as_str()).collect();
                    Ok(format!(
                        "Skill '{}' not found. Available: {}",
                        skill_name,
                        if available.is_empty() { "none".to_string() } else { available.join(", ") }
                    ))
                }
            }
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
```

Add to `tests/skill/mod.rs`:
```rust
mod tool_test;
```

- [ ] **Step 3: Run tests**

Run: `cargo test skill -v`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/skill/tool.rs tests/skill/tool_test.rs tests/skill/mod.rs
git commit -m "feat(skill): add SkillTool implementing Tool trait"
```

---

## Task 3: Agent Integration

**Files:**
- Modify: `src/agent/agent.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Add SkillRegistry to Agent struct**

In `src/agent/agent.rs`, add field to Agent struct and initialize in constructor:

```rust
// Add to Agent struct fields:
skill_registry: Arc<SkillRegistry>,

// In Agent::new() or Agent::for_test(), before Ok(Agent { ... }):
let home = std::env::var("HOME").unwrap_or_default();
let user_skills = format!("{}/.viv/skills", home);
let project_skills = ".viv/skills".to_string();
let skill_registry = Arc::new(SkillRegistry::load(&user_skills, &project_skills));

// Register SkillTool:
tools.register(Box::new(SkillTool::new(skill_registry.clone())));
```

- [ ] **Step 2: Pass skill_list to build_system_prompt**

In `handle_input()`, change line 256 from:
```rust
let system = build_system_prompt("", "", &memories, &mut self.prompt_cache);
```
to:
```rust
let skill_list = self.skill_registry.format_for_prompt();
let system = build_system_prompt("", &skill_list, &memories, &mut self.prompt_cache);
```

- [ ] **Step 3: Add imports**

Add to agent.rs:
```rust
use crate::skill::SkillRegistry;
use crate::skill::tool::SkillTool;
```

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: compiles, all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/agent/agent.rs src/tools/mod.rs
git commit -m "feat(skill): wire SkillRegistry into Agent and register SkillTool"
```
