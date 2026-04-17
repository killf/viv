use crate::json::JsonValue;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;
    fn execute(&self, input: &JsonValue) -> crate::Result<String>;
    fn permission_level(&self) -> PermissionLevel;
}

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionLevel {
    ReadOnly,
    Write,
    Execute,
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: vec![] }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    pub fn to_api_json(&self) -> String {
        let tools: Vec<String> = self.tools.iter().map(|t| {
            format!(
                "{{\"name\":{},\"description\":{},\"input_schema\":{}}}",
                JsonValue::Str(t.name().into()),
                JsonValue::Str(t.description().into()),
                t.input_schema(),
            )
        }).collect();
        format!("[{}]", tools.join(","))
    }

    pub fn default_tools(llm: std::sync::Arc<crate::llm::LLMClient>) -> Self {
        let _ = llm; // populated in Task 8
        ToolRegistry::new()
    }
}

pub mod bash;
pub mod file;
pub mod todo;
pub mod web;
