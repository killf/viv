use crate::agent::evolution::evolve_from_session;
use crate::agent::message::{ContentBlock, Message, PromptCache};
use crate::agent::prompt::{build_system_prompt, SystemPrompt};
use crate::agent::protocol::{AgentEvent, AgentMessage};
use crate::config::ConfigPaths;
use crate::core::json::JsonValue;
use crate::core::runtime::channel::AsyncReceiver;
use crate::core::sync::lock_or_recover;
use crate::llm::{LLMClient, LLMConfig, ModelTier};
use crate::lsp::LspManager;
use crate::lsp::config::LspConfig;
use crate::lsp::tools::{LspDefinitionTool, LspDiagnosticsTool, LspHoverTool, LspReferencesTool};
use crate::mcp::McpManager;
use crate::mcp::config::McpConfig;
use crate::mcp::tools::{
    GetMcpPromptTool, ListMcpPromptsTool, ListMcpResourcesTool, McpToolProxy, ReadMcpResourceTool,
};
use crate::memory::compaction::{compact_if_needed, estimate_tokens};
use crate::memory::index::MemoryIndex;
use crate::memory::retrieval::retrieve_relevant;
use crate::memory::store::MemoryStore;
use crate::permissions::PermissionManager;
use crate::skill::SkillRegistry;
use crate::skill::tool::SkillTool;
use crate::tools::{PermissionLevel, ToolRegistry};
use crate::{Result};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

// ── PermissionMode ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub enum PermissionMode {
    Default,
    Auto,
    Bypass,
}

// ── AgentConfig ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentConfig {
    pub model_tier: ModelTier,
    pub max_iterations: usize,
    pub top_k_memory: usize,
    pub permission_mode: PermissionMode,
    pub memory_dir: std::path::PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model_tier: ModelTier::Medium,
            max_iterations: 50,
            top_k_memory: 5,
            permission_mode: PermissionMode::Default,
            memory_dir: std::path::PathBuf::from(".viv/memory"),
        }
    }
}

// ── Agent ─────────────────────────────────────────────────────────────────────

pub struct Agent {
    messages: Vec<Message>,
    prompt_cache: PromptCache,
    llm: Arc<LLMClient>,
    store: Arc<MemoryStore>,
    index: Arc<Mutex<MemoryIndex>>,
    tools: ToolRegistry,
    permissions: PermissionManager,
    config: AgentConfig,
    input_tokens: u64,
    output_tokens: u64,
    event_rx: AsyncReceiver<AgentEvent>,
    msg_tx: Sender<AgentMessage>,
    mcp: Arc<Mutex<McpManager>>,
    lsp: Arc<Mutex<LspManager>>,
    skill_registry: Arc<SkillRegistry>,
}

impl Agent {
    /// Create a sub-agent for running delegated tasks.
    ///
    /// Unlike `new()`, this skips `LLMConfig::from_env()`, MCP/LSP initialization, and memory loading.
    /// The sub-agent shares the parent's LLM client and uses a temporary memory directory.
    pub async fn new_sub(
        config: AgentConfig,
        endpoint: crate::agent::channel::AgentEndpoint,
        llm: Arc<LLMClient>,
    ) -> Result<Self> {
        let tools = ToolRegistry::default_tools_without("Agent", Arc::clone(&llm));

        let mcp_config = McpConfig { servers: vec![] };
        let mcp = Arc::new(Mutex::new(McpManager::from_config(&mcp_config).await));
        let lsp_config = LspConfig { servers: vec![] };
        let lsp = Arc::new(Mutex::new(LspManager::new(lsp_config)));

        let memory_dir = config.memory_dir.clone();
        let store = Arc::new(MemoryStore::new(memory_dir)?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));

        let skill_registry = Arc::new(SkillRegistry::new());

        Ok(Agent {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            tools,
            permissions: PermissionManager::default(),
            config,
            input_tokens: 0,
            output_tokens: 0,
            event_rx: endpoint.rx,
            msg_tx: endpoint.tx,
            mcp,
            lsp,
            skill_registry,
        })
    }

    pub async fn new(
        config: AgentConfig,
        event_rx: AsyncReceiver<AgentEvent>,
        msg_tx: Sender<AgentMessage>,
    ) -> Result<Self> {
        // Cascading config paths: `.viv/settings.json` > `~/.viv/settings.json`
        let config_paths = ConfigPaths::new();

        // Model config: project → user → env vars (handled inside from_env)
        let model_config = config_paths.model_config()?;
        let llm_config = LLMConfig::from_env(&model_config)?;
        let model_name = llm_config.model(config.model_tier.clone()).to_string();
        let llm = Arc::new(LLMClient::new(llm_config));
        let store = Arc::new(MemoryStore::new(config.memory_dir.clone())?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        let mut tools = ToolRegistry::default_tools(Arc::clone(&llm));

        // Load MCP config (project first, fall back to user home)
        let mcp_config = match config_paths.settings_path() {
            Some(path) => McpConfig::load(path.to_string_lossy().as_ref())?,
            None => McpConfig { servers: vec![] },
        };

        // Connect to MCP servers
        let mcp_manager = McpManager::from_config(&mcp_config).await;
        let mcp = Arc::new(Mutex::new(mcp_manager));

        // Register MCP tool proxies
        {
            let mgr = lock_or_recover(&mcp);
            for handle in &mgr.servers {
                for tool in &handle.tools {
                    tools.register(Box::new(McpToolProxy::new(
                        &handle.name,
                        &tool.name,
                        tool.description.as_deref().unwrap_or(""),
                        tool.input_schema.clone(),
                        mcp.clone(),
                    )));
                }
            }
        }
        tools.register(Box::new(ListMcpResourcesTool::new(mcp.clone())));
        tools.register(Box::new(ReadMcpResourceTool::new(mcp.clone())));
        tools.register(Box::new(ListMcpPromptsTool::new(mcp.clone())));
        tools.register(Box::new(GetMcpPromptTool::new(mcp.clone())));

        // Load LSP config (reuse same cascading settings path)
        let lsp_config = match config_paths.settings_path() {
            Some(path) => LspConfig::load(path.to_string_lossy().as_ref())?,
            None => LspConfig { servers: vec![] },
        };
        let lsp = Arc::new(Mutex::new(LspManager::new(lsp_config)));

        // Register LSP tools
        tools.register(Box::new(LspDefinitionTool::new(lsp.clone())));
        tools.register(Box::new(LspReferencesTool::new(lsp.clone())));
        tools.register(Box::new(LspHoverTool::new(lsp.clone())));
        tools.register(Box::new(LspDiagnosticsTool::new(lsp.clone())));

        // Load skills: project `.viv/skills` takes priority over user `~/.viv/skills`
        let skills_dirs = config_paths.skills_dirs();
        let project_skills = skills_dirs.first().map(|p| p.to_string_lossy().into_owned());
        let user_skills = skills_dirs.get(1).map(|p| p.to_string_lossy().into_owned());
        let skill_registry = Arc::new(SkillRegistry::load(
            user_skills.as_deref().unwrap_or("~/.viv/skills"),
            project_skills.as_deref().unwrap_or(".viv/skills"),
        ));
        tools.register(Box::new(SkillTool::new(skill_registry.clone())));

        let _ = msg_tx.send(AgentMessage::Ready { model: model_name });

        Ok(Agent {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            tools,
            permissions: PermissionManager::default(),
            config,
            input_tokens: 0,
            output_tokens: 0,
            event_rx,
            msg_tx,
            mcp,
            lsp,
            skill_registry,
        })
    }

    /// Infinite loop: reads from event_rx until Quit or channel close.
    pub async fn run(mut self) -> Result<()> {
        loop {
            match self.event_rx.recv().await {
                Ok(AgentEvent::Input(text)) => {
                    if text.trim() == "/exit" {
                        self.shutdown().await?;
                        break;
                    }
                    if let Err(e) = self.handle_input(text).await {
                        let _ = self.msg_tx.send(AgentMessage::Error(e.to_string()));
                        let _ = self.msg_tx.send(AgentMessage::Done);
                    }
                }
                Ok(AgentEvent::Quit) => {
                    self.shutdown().await?;
                    break;
                }
                Ok(AgentEvent::Interrupt) | Ok(AgentEvent::PermissionResponse(_)) => {}
                Err(_) => break,
            }
        }
        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    async fn shutdown(&mut self) -> Result<()> {
        self.evolve().await?;
        // Take servers out of mutex to avoid holding guard across awaits
        let mut mgr = lock_or_recover(&self.mcp);
        let servers = std::mem::take(&mut mgr.servers);
        drop(mgr);
        for mut handle in servers {
            let _ = handle.shutdown().await;
        }
        // Shutdown LSP servers
        {
            let mut lsp_mgr = lock_or_recover(&self.lsp);
            lsp_mgr.shutdown_all().await;
        }
        let _ = self.msg_tx.send(AgentMessage::Evolved);
        Ok(())
    }

    // Safe: single-threaded runtime (block_on_local), MutexGuard never crosses threads
    #[allow(clippy::await_holding_lock)]
    async fn handle_input(&mut self, text: String) -> Result<()> {
        let _ = self.msg_tx.send(AgentMessage::Thinking);

        let memories = {
            let idx = lock_or_recover(&self.index);
            let results = retrieve_relevant(
                &text,
                &idx,
                &self.store,
                &self.llm,
                self.config.top_k_memory,
            )
            .await;
            drop(idx);
            match results {
                Ok(m) => {
                    let _ = self
                        .msg_tx
                        .send(AgentMessage::Status(format!("检索记忆({} 条)…", m.len())));
                    m
                }
                Err(_) => vec![],
            }
        };

        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let skill_list = self.skill_registry.format_for_prompt();
        let system = build_system_prompt(&cwd, "", &skill_list, &memories, &mut self.prompt_cache);
        self.messages.push(Message::user_text(text));

        let token_estimate = estimate_tokens(&self.messages);
        compact_if_needed(
            &mut self.messages,
            token_estimate,
            100_000,
            10,
            self.llm.as_ref(),
        )
        .await?;

        self.agentic_loop(system).await?;

        let _ = self.msg_tx.send(AgentMessage::Tokens {
            input: self.input_tokens,
            output: self.output_tokens,
        });
        let _ = self.msg_tx.send(AgentMessage::Done);
        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    async fn agentic_loop(&mut self, system: SystemPrompt) -> Result<()> {
        let tools_json = self.tools.to_api_json();

        for _ in 0..self.config.max_iterations {
            let msg_tx = self.msg_tx.clone();
            let stream_result = self
                .llm
                .stream_agent_async(
                    &system.blocks,
                    &self.messages,
                    &tools_json,
                    self.config.model_tier.clone(),
                    move |chunk| {
                        let _ = msg_tx.send(AgentMessage::TextChunk(chunk.to_string()));
                    },
                )
                .await?;

            self.input_tokens += stream_result.input_tokens;
            self.output_tokens += stream_result.output_tokens;

            let has_tool_uses = !stream_result.tool_uses.is_empty();

            let mut assistant_blocks = stream_result.text_blocks;
            assistant_blocks.extend(stream_result.tool_uses.clone());
            self.messages.push(Message::Assistant(assistant_blocks));

            if !has_tool_uses || stream_result.stop_reason == "end_turn" {
                break;
            }

            let tool_uses = stream_result.tool_uses;
            let mut tool_results = Vec::new();

            // Partition tool calls: Agent tools run concurrently, others run serially
            let mut normal_uses = Vec::new();
            let mut agent_uses = Vec::new();
            for tu in &tool_uses {
                if let ContentBlock::ToolUse { name, .. } = tu {
                    if name == "Agent" {
                        agent_uses.push(tu);
                    } else {
                        normal_uses.push(tu);
                    }
                }
            }

            // Execute normal tools serially (they may have dependencies on each other)
            for tu in &normal_uses {
                if let ContentBlock::ToolUse { id, name, input } = tu {
                    let allowed = self.check_permission(name, input).await?;

                    let result = if allowed {
                        match self.tools.get(name) {
                            None => Err(crate::Error::Tool(format!("unknown tool: {}", name))),
                            Some(tool) => {
                                let _ = self.msg_tx.send(AgentMessage::ToolStart {
                                    name: name.clone(),
                                    input: format_tool_input(input),
                                });
                                tool.execute(input).await
                            }
                        }
                    } else {
                        Err(crate::Error::Tool("permission denied".into()))
                    };

                    let (content, is_error) = match &result {
                        Ok(out) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolEnd {
                                name: name.clone(),
                                output: out.chars().take(200).collect(),
                            });
                            (out.clone(), false)
                        }
                        Err(e) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolError {
                                name: name.clone(),
                                error: e.to_string(),
                            });
                            (e.to_string(), true)
                        }
                    };

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: vec![ContentBlock::Text(content)],
                        is_error,
                    });
                }
            }

            // Execute Agent tools concurrently using join_all
            if !agent_uses.is_empty() {
                use crate::core::runtime::join_all;

                // Check permissions first (serially, since check_permission borrows &mut self)
                let mut approved: Vec<(&ContentBlock, bool)> = Vec::new();
                for tu in &agent_uses {
                    if let ContentBlock::ToolUse { name, input, .. } = tu {
                        let allowed = self.check_permission(name, input).await?;
                        approved.push((tu, allowed));
                    }
                }

                // Build futures for approved agent calls
                type AgentResult = (String, String, std::result::Result<String, crate::Error>);
                type AgentFuture =
                    std::pin::Pin<Box<dyn std::future::Future<Output = AgentResult> + Send>>;
                let mut futures: Vec<AgentFuture> = Vec::new();
                let mut denied_results: Vec<AgentResult> = Vec::new();

                for (tu, allowed) in &approved {
                    if let ContentBlock::ToolUse { id, name, input } = tu {
                        if *allowed {
                            let tool = self.tools.get(name);
                            match tool {
                                None => {
                                    denied_results.push((
                                        id.clone(),
                                        name.clone(),
                                        Err(crate::Error::Tool(format!("unknown tool: {}", name))),
                                    ));
                                }
                                Some(tool) => {
                                    let msg_tx = self.msg_tx.clone();
                                    let id = id.clone();
                                    let name = name.clone();
                                    let input = input.clone();
                                    let formatted = format_tool_input(&input);
                                    let fut = tool.execute(&input);
                                    let _ = msg_tx.send(AgentMessage::ToolStart {
                                        name: name.clone(),
                                        input: formatted,
                                    });
                                    // SAFETY: tool.execute returns Pin<Box<dyn Future + Send + '_>>,
                                    // but lifetime is tied to `tool` which lives in self.tools.
                                    // The tool registry won't be modified during execution.
                                    // We need to erase the lifetime for join_all.
                                    type ErasedFut = std::pin::Pin<
                                        Box<
                                            dyn std::future::Future<
                                                    Output = std::result::Result<
                                                        String,
                                                        crate::Error,
                                                    >,
                                                > + Send
                                                + 'static,
                                        >,
                                    >;
                                    let fut: ErasedFut = unsafe { std::mem::transmute(fut) };
                                    futures.push(Box::pin(async move {
                                        let result = fut.await;
                                        (id, name, result)
                                    }));
                                }
                            }
                        } else {
                            denied_results.push((
                                id.clone(),
                                name.clone(),
                                Err(crate::Error::Tool("permission denied".into())),
                            ));
                        }
                    }
                }

                // Collect denied results first
                for (id, name, result) in denied_results {
                    let (content, is_error) = match &result {
                        Ok(out) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolEnd {
                                name: name.clone(),
                                output: out.chars().take(200).collect(),
                            });
                            (out.clone(), false)
                        }
                        Err(e) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolError {
                                name: name.clone(),
                                error: e.to_string(),
                            });
                            (e.to_string(), true)
                        }
                    };
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: vec![ContentBlock::Text(content)],
                        is_error,
                    });
                }

                // Run approved agent tools concurrently
                if !futures.is_empty() {
                    let results = join_all(futures).await;
                    for (id, name, result) in results {
                        let (content, is_error) = match &result {
                            Ok(out) => {
                                let _ = self.msg_tx.send(AgentMessage::ToolEnd {
                                    name: name.clone(),
                                    output: out.chars().take(200).collect(),
                                });
                                (out.clone(), false)
                            }
                            Err(e) => {
                                let _ = self.msg_tx.send(AgentMessage::ToolError {
                                    name: name.clone(),
                                    error: e.to_string(),
                                });
                                (e.to_string(), true)
                            }
                        };
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: vec![ContentBlock::Text(content)],
                            is_error,
                        });
                    }
                }
            }

            // After all tool results are collected, notify LSP of any file changes.
            for tu in &tool_uses {
                if let ContentBlock::ToolUse { name, input, .. } = tu {
                    if matches!(name.as_str(), "Edit" | "Write" | "MultiEdit") {
                        if let Some(path) = input
                            .get("file_path")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                        {
                            let mut mgr = lock_or_recover(&self.lsp);
                            // Safe: single-threaded runtime, guard does not cross threads.
                            #[allow(clippy::await_holding_lock)]
                            if let Err(e) = mgr.notify_did_change(&path).await {
                                eprintln!("[agent] failed to notify LSP of file change: {}", e);
                            }
                        }
                    }
                }
            }

            self.messages.push(Message::User(tool_results));
        }

        Ok(())
    }

    async fn check_permission(&mut self, tool_name: &str, input: &JsonValue) -> Result<bool> {
        if self.config.permission_mode == PermissionMode::Bypass {
            return Ok(true);
        }

        let is_readonly = self
            .tools
            .get(tool_name)
            .map(|t| t.permission_level() == PermissionLevel::ReadOnly)
            .unwrap_or(false);

        if is_readonly {
            return Ok(true);
        }

        if self.permissions.is_allowed(tool_name) {
            return Ok(true);
        }

        if self.config.permission_mode == PermissionMode::Auto {
            self.permissions.grant(tool_name);
            return Ok(true);
        }

        let _ = self.msg_tx.send(AgentMessage::PermissionRequest {
            tool: tool_name.to_string(),
            input: format_tool_input(input),
        });

        loop {
            match self.event_rx.recv().await {
                Ok(AgentEvent::PermissionResponse(allowed)) => {
                    if allowed {
                        self.permissions.grant(tool_name);
                    }
                    return Ok(allowed);
                }
                Ok(AgentEvent::Interrupt) => return Ok(false),
                Ok(_) => continue,
                Err(_) => return Ok(false),
            }
        }
    }

    #[allow(clippy::await_holding_lock)]
    async fn evolve(&mut self) -> Result<()> {
        let mut idx = lock_or_recover(&self.index);
        evolve_from_session(&self.messages, &self.store, &mut idx, &self.llm).await?;
        Ok(())
    }
}

fn format_tool_input(input: &JsonValue) -> String {
    match input {
        JsonValue::Object(pairs) => pairs
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    JsonValue::Str(s) => {
                        format!("\"{}\"", s.chars().take(40).collect::<String>())
                    }
                    other => format!("{}", other).chars().take(40).collect::<String>(),
                };
                format!("{}={}", k, val)
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}
