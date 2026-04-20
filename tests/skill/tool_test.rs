use std::sync::Arc;
use viv::core::json::JsonValue;
use viv::skill::tool::SkillTool;
use viv::skill::{SkillEntry, SkillRegistry, SkillSource};
use viv::tools::{poll_to_completion, Tool};

fn make_registry_with_skills() -> Arc<SkillRegistry> {
    let mut reg = SkillRegistry::new();
    reg.add(SkillEntry {
        name: "commit".to_string(),
        description: "Use when committing code".to_string(),
        when_to_use: "".to_string(),
        content: "Step 1: run tests\nStep 2: commit".to_string(),
        base_dir: "/skills/commit".to_string(),
        source: SkillSource::User,
    });
    reg.add(SkillEntry {
        name: "review".to_string(),
        description: "Use when reviewing PRs".to_string(),
        when_to_use: "".to_string(),
        content: "Review instructions here.".to_string(),
        base_dir: "/skills/review".to_string(),
        source: SkillSource::Project,
    });
    Arc::new(reg)
}

#[test]
fn skill_tool_name() {
    let registry = Arc::new(SkillRegistry::new());
    let tool = SkillTool::new(registry);
    assert_eq!(tool.name(), "Skill");
}

#[test]
fn skill_tool_description() {
    let registry = Arc::new(SkillRegistry::new());
    let tool = SkillTool::new(registry);
    assert!(!tool.description().is_empty());
}

#[test]
fn skill_tool_found() {
    let registry = make_registry_with_skills();
    let tool = SkillTool::new(registry);

    let input = JsonValue::parse(r#"{"skill": "commit"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input)).unwrap();

    assert!(
        result.contains("Base directory"),
        "should contain 'Base directory', got: {result}"
    );
    assert!(
        result.contains("/skills/commit"),
        "should contain the base dir path, got: {result}"
    );
    assert!(
        result.contains("Step 1: run tests"),
        "should contain skill content, got: {result}"
    );
}

#[test]
fn skill_tool_not_found() {
    let registry = make_registry_with_skills();
    let tool = SkillTool::new(registry);

    let input = JsonValue::parse(r#"{"skill": "nonexistent"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input)).unwrap();

    assert!(
        result.contains("not found"),
        "should contain 'not found', got: {result}"
    );
    assert!(
        result.contains("commit"),
        "should list available skills, got: {result}"
    );
    assert!(
        result.contains("review"),
        "should list available skills, got: {result}"
    );
}

#[test]
fn skill_tool_with_args() {
    let registry = make_registry_with_skills();
    let tool = SkillTool::new(registry);

    let input = JsonValue::parse(r#"{"skill": "review", "args": "some extra arguments"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input)).unwrap();

    // Args are accepted without error; content is still returned.
    assert!(
        result.contains("Base directory"),
        "should contain 'Base directory', got: {result}"
    );
    assert!(
        result.contains("Review instructions here"),
        "should contain skill content, got: {result}"
    );
}

#[test]
fn skill_tool_missing_skill_field_returns_error() {
    let registry = make_registry_with_skills();
    let tool = SkillTool::new(registry);

    let input = JsonValue::parse(r#"{"args": "no skill key"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input));
    assert!(result.is_err(), "should return error when 'skill' field is missing");
}

#[test]
fn skill_tool_empty_registry_not_found() {
    let registry = Arc::new(SkillRegistry::new());
    let tool = SkillTool::new(registry);

    let input = JsonValue::parse(r#"{"skill": "anything"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input)).unwrap();

    assert!(
        result.contains("not found"),
        "should contain 'not found', got: {result}"
    );
}
