use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use crate::Result;
use crate::agent::evolution::evolve_from_session;
use crate::agent::message::{ContentBlock, Message, PromptCache};
use crate::agent::prompt::{SystemPrompt, build_system_prompt};
use crate::bus::{AgentEvent, AgentMessage};
use crate::core::json::JsonValue;
use crate::core::runtime::channel::AsyncReceiver;
use crate::llm::{LLMClient, LLMConfig, ModelTier};
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
use crate::tools::{PermissionLevel, ToolRegistry};

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
}

impl Agent {
    pub async fn new(
        config: AgentConfig,
        event_rx: AsyncReceiver<AgentEvent>,
        msg_tx: Sender<AgentMessage>,
    ) -> Result<Self> {
        let llm_config = LLMConfig::from_env()?;
        let model_name = llm_config.model(config.model_tier.clone()).to_string();
        let llm = Arc::new(LLMClient::new(llm_config));
        let store = Arc::new(MemoryStore::new(config.memory_dir.clone())?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        let mut tools = ToolRegistry::default_tools(Arc::clone(&llm));

        // Load MCP config (if file doesn't exist, returns empty config)
        let mcp_config = McpConfig::load(".viv/settings.json")?;

        // Connect to MCP servers
        let mcp_manager = McpManager::from_config(&mcp_config).await;
        let mcp = Arc::new(Mutex::new(mcp_manager));

        // Register MCP tool proxies
        {
            let mgr = mcp.lock().unwrap();
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

    async fn shutdown(&mut self) -> Result<()> {
        self.evolve().await?;
        // Take servers out of mutex to avoid holding guard across awaits
        let mut mgr = self.mcp.lock().unwrap();
        let servers = std::mem::take(&mut mgr.servers);
        drop(mgr);
        for mut handle in servers {
            let _ = handle.shutdown().await;
        }
        let _ = self.msg_tx.send(AgentMessage::Evolved);
        Ok(())
    }

    async fn handle_input(&mut self, text: String) -> Result<()> {
        let _ = self.msg_tx.send(AgentMessage::Thinking);

        let memories = {
            let idx = self.index.lock().unwrap();
            let results = retrieve_relevant(
                &text,
                &idx,
                &self.store,
                &self.llm,
                self.config.top_k_memory,
            ).await;
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

        let system = build_system_prompt("", "", &memories, &mut self.prompt_cache);
        self.messages.push(Message::user_text(text));

        let token_estimate = estimate_tokens(&self.messages);
        compact_if_needed(
            &mut self.messages,
            token_estimate,
            100_000,
            10,
            self.llm.as_ref(),
        ).await?;

        self.agentic_loop(system).await?;

        let _ = self.msg_tx.send(AgentMessage::Tokens {
            input: self.input_tokens,
            output: self.output_tokens,
        });
        let _ = self.msg_tx.send(AgentMessage::Done);
        Ok(())
    }

    async fn agentic_loop(&mut self, system: SystemPrompt) -> Result<()> {
        let tools_json = self.tools.to_api_json();

        for _ in 0..self.config.max_iterations {
            let msg_tx = self.msg_tx.clone();
            let stream_result = self.llm.stream_agent_async(
                &system.blocks,
                &self.messages,
                &tools_json,
                self.config.model_tier.clone(),
                move |chunk| {
                    let _ = msg_tx.send(AgentMessage::TextChunk(chunk.to_string()));
                },
            ).await?;

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

            for tu in &tool_uses {
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

    async fn evolve(&mut self) -> Result<()> {
        let mut idx = self.index.lock().unwrap();
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
