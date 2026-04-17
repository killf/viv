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
        use crate::tools::bash::BashTool;
        use crate::tools::file::edit::{EditTool, MultiEditTool};
        use crate::tools::file::glob::GlobTool;
        use crate::tools::file::grep::GrepTool;
        use crate::tools::file::ls::LsTool;
        use crate::tools::file::read::ReadTool;
        use crate::tools::file::write::WriteTool;
        use crate::tools::todo::{TodoReadTool, TodoWriteTool};
        use crate::tools::web::WebFetchTool;

        let todo_path = std::path::PathBuf::from(".viv/todo.json");
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(BashTool));
        reg.register(Box::new(ReadTool));
        reg.register(Box::new(WriteTool));
        reg.register(Box::new(EditTool));
        reg.register(Box::new(MultiEditTool));
        reg.register(Box::new(GlobTool));
        reg.register(Box::new(GrepTool));
        reg.register(Box::new(LsTool));
        reg.register(Box::new(TodoWriteTool::new(todo_path.clone())));
        reg.register(Box::new(TodoReadTool::new(todo_path)));
        reg.register(Box::new(WebFetchTool::new(llm)));
        reg
    }
}

pub mod bash;
pub mod file;
pub mod todo;
pub mod web;
