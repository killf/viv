use viv::tools::sub_agent::SubAgentTool;
use viv::tools::{PermissionLevel, Tool, ToolRegistry};

fn make_llm() -> Option<std::sync::Arc<viv::llm::LLMClient>> {
    let config = viv::llm::LLMConfig::from_env().ok()?;
    Some(std::sync::Arc::new(viv::llm::LLMClient::new(config)))
}

#[test]
fn sub_agent_tool_has_correct_name_and_permission() {
    let llm = match make_llm() {
        Some(l) => l,
        None => return,
    };
    let tool = SubAgentTool::new(llm);
    assert_eq!(tool.name(), "Agent");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);
}

#[test]
fn sub_agent_schema_requires_prompt() {
    let llm = match make_llm() {
        Some(l) => l,
        None => return,
    };
    let tool = SubAgentTool::new(llm);
    let schema = tool.input_schema();
    let required = schema.get("required").unwrap().as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("prompt")));
}

#[test]
fn sub_agent_schema_has_model_and_max_iterations() {
    let llm = match make_llm() {
        Some(l) => l,
        None => return,
    };
    let tool = SubAgentTool::new(llm);
    let schema = tool.input_schema();
    let props = schema.get("properties").unwrap();
    assert!(props.get("prompt").is_some());
    assert!(props.get("model").is_some());
    assert!(props.get("max_iterations").is_some());
}

#[test]
fn default_tools_includes_agent() {
    let llm = match make_llm() {
        Some(l) => l,
        None => return,
    };
    let reg = ToolRegistry::default_tools(llm);
    assert!(reg.get("Agent").is_some(), "Expected 'Agent' tool");
}

#[test]
fn default_tools_without_agent_excludes_it() {
    let llm = match make_llm() {
        Some(l) => l,
        None => return,
    };
    let reg = ToolRegistry::default_tools_without("Agent", llm);
    assert!(reg.get("Agent").is_none(), "Agent should be excluded");
    // Other tools should still be present
    assert!(reg.get("Read").is_some(), "Read should still be present");
    assert!(reg.get("Bash").is_some(), "Bash should still be present");
}
