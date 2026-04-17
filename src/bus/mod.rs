pub mod terminal;

/// UI 线程 → Agent 线程
#[derive(Debug)]
pub enum AgentEvent {
    Input(String),
    PermissionResponse(bool),
    Interrupt,
    Quit,
}

/// Agent 线程 → UI 线程
#[derive(Debug)]
pub enum AgentMessage {
    Ready { model: String },
    Thinking,
    TextChunk(String),
    ToolStart { name: String, input: String },
    ToolEnd { name: String, output: String },
    ToolError { name: String, error: String },
    PermissionRequest { tool: String, input: String },
    Status(String),
    Tokens { input: u64, output: u64 },
    Done,
    Evolved,
    Error(String),
}
