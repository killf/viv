use std::fs;
use viv::skill::{SkillEntry, SkillRegistry, SkillSource, parse_frontmatter};

// ── parse_frontmatter ────────────────────────────────────────────────────────

#[test]
fn parse_frontmatter_basic() {
    let content = "---\nname: commit\ndescription: Use when committing\n---\nBody text here\n";
    let result = parse_frontmatter(content);
    assert!(result.is_some());
    let (fields, body) = result.unwrap();
    assert_eq!(fields.get("name").map(|s| s.as_str()), Some("commit"));
    assert_eq!(
        fields.get("description").map(|s| s.as_str()),
        Some("Use when committing")
    );
    assert_eq!(body.trim(), "Body text here");
}

#[test]
fn parse_frontmatter_no_frontmatter() {
    let content = "No frontmatter here\nJust regular content\n";
    let result = parse_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn parse_frontmatter_empty_body() {
    let content = "---\nname: test\ndescription: A test skill\n---\n";
    let result = parse_frontmatter(content);
    assert!(result.is_some());
    let (fields, body) = result.unwrap();
    assert_eq!(fields.get("name").map(|s| s.as_str()), Some("test"));
    assert_eq!(fields.get("description").map(|s| s.as_str()), Some("A test skill"));
    assert_eq!(body.trim(), "");
}

// ── SkillRegistry ─────────────────────────────────────────────────────────────

#[test]
fn registry_empty() {
    let reg = SkillRegistry::new();
    assert!(reg.is_empty());
    assert!(reg.get("anything").is_none());
    assert_eq!(reg.list().len(), 0);
}

#[test]
fn registry_add_and_get() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".to_string(),
        description: "Use when committing".to_string(),
        content: "Commit instructions here.".to_string(),
        base_dir: "/some/dir".to_string(),
        source: SkillSource::User,
    });
    assert!(!reg.is_empty());
    let entry = reg.get("commit");
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.name, "commit");
    assert_eq!(entry.description, "Use when committing");
    assert_eq!(entry.source, SkillSource::User);
}

#[test]
fn registry_project_overrides_user() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".to_string(),
        description: "User version".to_string(),
        content: "User content".to_string(),
        base_dir: "/user/dir".to_string(),
        source: SkillSource::User,
    });
    reg.add(SkillEntry {
        name: "commit".to_string(),
        description: "Project version".to_string(),
        content: "Project content".to_string(),
        base_dir: "/project/dir".to_string(),
        source: SkillSource::Project,
    });
    assert_eq!(reg.list().len(), 1);
    let entry = reg.get("commit").unwrap();
    assert_eq!(entry.description, "Project version");
    assert_eq!(entry.source, SkillSource::Project);
}

// ── format_for_prompt ─────────────────────────────────────────────────────────

#[test]
fn format_for_prompt_empty() {
    let reg = SkillRegistry::new();
    assert_eq!(reg.format_for_prompt(), "");
}

#[test]
fn format_for_prompt_lists_skills() {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".to_string(),
        description: "Use when committing".to_string(),
        content: "".to_string(),
        base_dir: "/some/dir".to_string(),
        source: SkillSource::User,
    });
    reg.add(SkillEntry {
        name: "review".to_string(),
        description: "Use when reviewing code".to_string(),
        content: "".to_string(),
        base_dir: "/some/dir".to_string(),
        source: SkillSource::User,
    });
    let prompt = reg.format_for_prompt();
    assert!(prompt.contains("commit"), "should contain skill name 'commit'");
    assert!(prompt.contains("Use when committing"), "should contain description");
    assert!(prompt.contains("review"), "should contain skill name 'review'");
    assert!(prompt.contains("Use when reviewing code"), "should contain review description");
    assert!(prompt.contains("Available skills"), "should have header");
}

// ── load_from_dir ─────────────────────────────────────────────────────────────

#[test]
fn load_from_directory() {
    let tmp = "/tmp/viv_skill_test_load";
    let skill_dir = format!("{}/commit", tmp);
    fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = format!("{}/SKILL.md", skill_dir);
    fs::write(
        &skill_md,
        "---\nname: commit\ndescription: Use when committing\n---\nCommit instructions.\n",
    )
    .unwrap();

    let reg = SkillRegistry::load_from_dir(tmp, SkillSource::User);
    assert!(!reg.is_empty());
    let entry = reg.get("commit");
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.name, "commit");
    assert_eq!(entry.description, "Use when committing");
    assert!(entry.content.contains("Commit instructions"));
    assert_eq!(entry.source, SkillSource::User);

    // clean up
    fs::remove_dir_all(tmp).ok();
}

#[test]
fn load_skips_dir_without_skill_md() {
    let tmp = "/tmp/viv_skill_test_skip";
    let no_skill_dir = format!("{}/no_skill", tmp);
    fs::create_dir_all(&no_skill_dir).unwrap();
    // no SKILL.md in no_skill_dir

    let reg = SkillRegistry::load_from_dir(tmp, SkillSource::User);
    assert!(reg.is_empty());

    // clean up
    fs::remove_dir_all(tmp).ok();
}
