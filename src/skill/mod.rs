use std::collections::HashMap;
use std::path::PathBuf;

pub mod tool;

/// Where a skill was loaded from.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillSource {
    /// Skill shipped with viv itself.
    Builtin,
    /// Skill installed in the user's global config dir.
    User,
    /// Skill found in the current project directory.
    Project,
}

/// A single skill entry stored in the registry.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// Unique skill name (e.g. `"commit"`, `"review-pr"`).
    pub name: String,
    /// One-line description shown in system prompt listing.
    pub description: String,
    /// Full Markdown content of the skill.
    pub content: String,
    /// Base directory associated with the skill (string path, used for
    /// relative file operations performed while executing the skill).
    pub base_dir: String,
    /// Where this skill was loaded from.
    pub source: SkillSource,
}

/// Parse YAML-style front-matter from skill Markdown content.
///
/// Returns `Some((fields, body))` if the content starts with `---\n`.
/// `fields` maps key → value strings. `body` is the content after the
/// closing `---` delimiter.
pub fn parse_frontmatter(content: &str) -> Option<(HashMap<String, String>, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let fm_text = &rest[..end];
    let body = &rest[end + 4..]; // skip "\n---"

    let mut fields = HashMap::new();
    for line in fm_text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Some((fields, body))
}

/// Registry that holds all known skills.
#[derive(Debug, Default)]
pub struct SkillRegistry {
    /// Skills keyed by name. Later insertions for the same name override
    /// earlier ones (Project > User > Builtin).
    skills: HashMap<String, SkillEntry>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        SkillRegistry {
            skills: HashMap::new(),
        }
    }

    /// Add a skill, overwriting any existing entry with the same name.
    pub fn add(&mut self, entry: SkillEntry) {
        self.skills.insert(entry.name.clone(), entry);
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&SkillEntry> {
        self.skills.get(name)
    }

    /// Return all skill entries sorted by name.
    pub fn list(&self) -> Vec<&SkillEntry> {
        let mut entries: Vec<&SkillEntry> = self.skills.values().collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    /// Return all skill names in sorted order.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.skills.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Return true if no skills are registered.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Format skills as a compact listing suitable for inclusion in a system
    /// prompt.  Returns an empty string when there are no skills.
    pub fn format_for_prompt(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }
        let mut out = String::from("Available skills:\n");
        for entry in self.list() {
            out.push_str(&format!("- {}: {}\n", entry.name, entry.description));
        }
        out
    }

    /// Load skills from a directory.  Each sub-directory that contains a
    /// `SKILL.md` file is treated as one skill.  The front-matter `name` and
    /// `description` fields are used if present; otherwise the directory name
    /// is used as the skill name.
    pub fn load_from_dir(dir: &str, source: SkillSource) -> Self {
        let mut reg = SkillRegistry::new();
        let path = PathBuf::from(dir);
        let read_dir = match std::fs::read_dir(&path) {
            Ok(rd) => rd,
            Err(_) => return reg,
        };
        for entry in read_dir.flatten() {
            let skill_dir = entry.path();
            if !skill_dir.is_dir() {
                continue;
            }
            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let raw = match std::fs::read_to_string(&skill_md) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let dir_name = skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let (name, description, content) =
                if let Some((fields, body)) = parse_frontmatter(&raw) {
                    let n = fields
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| dir_name.clone());
                    let d = fields.get("description").cloned().unwrap_or_default();
                    (n, d, body.to_string())
                } else {
                    (dir_name, String::new(), raw)
                };

            let base_dir = skill_dir.to_string_lossy().into_owned();
            reg.add(SkillEntry {
                name,
                description,
                content,
                base_dir,
                source: source.clone(),
            });
        }
        reg
    }
}
