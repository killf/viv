/// UI 线程 → Agent 线程
#[derive(Debug)]
pub enum AgentEvent {
    Input(String),
    SlashCommand(String),
    ColonCommand(String),
    PermissionResponse(PermissionResponse),
    Interrupt,
    Quit,
}

/// How the user responded to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResponse {
    /// Deny — refuse this operation.
    Deny,
    /// Allow — allow this time only.
    Allow,
    /// AlwaysAllow — remember and auto-allow similar operations.
    AlwaysAllow,
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
