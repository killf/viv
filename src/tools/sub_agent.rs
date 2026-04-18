use crate::bus::channel::agent_channel;
use crate::bus::{AgentEvent, AgentMessage};
use crate::core::json::JsonValue;
use crate::core::runtime::AssertSend;
use crate::core::runtime::join;
use crate::error::Error;
use crate::llm::{LLMClient, ModelTier};
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct SubAgentTool {
    llm: Arc<LLMClient>,
}

impl SubAgentTool {
    pub fn new(llm: Arc<LLMClient>) -> Self {
        SubAgentTool { llm }
    }
}

impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "Agent"
    }

    fn description(&self) -> &str {
        "Launch a new agent to handle complex, multi-step tasks.\n\nThe sub-agent runs with its own message history and tool set. It can use all tools except Agent itself (no recursion).\n\nUse this when you need to delegate an independent task that requires multiple tool calls."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(
            r#"{"type":"object","properties":{"prompt":{"type":"string","description":"The task for the sub-agent to perform"},"model":{"type":"string","description":"Model tier: fast, medium, or slow. Default: fast"},"max_iterations":{"type":"number","description":"Maximum agentic loop iterations. Default: 20"}},"required":["prompt"]}"#,
        )
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        let llm = Arc::clone(&self.llm);
        Box::pin(AssertSend(async move {
            let prompt = input
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'prompt'".into()))?;
            let tier = match input.get("model").and_then(|v| v.as_str()) {
                Some("medium") => ModelTier::Medium,
                Some("slow") => ModelTier::Slow,
                _ => ModelTier::Fast,
            };
            let max_iter = input
                .get("max_iterations")
                .and_then(|v| v.as_i64())
                .unwrap_or(20) as usize;

            run_sub_agent(&llm, prompt, tier, max_iter).await
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

async fn run_sub_agent(
    llm: &LLMClient,
    prompt: &str,
    tier: ModelTier,
    max_iter: usize,
) -> crate::Result<String> {
    use crate::agent::agent::{AgentConfig, PermissionMode};

    let (handle, endpoint) = agent_channel()?;

    let memory_dir = std::env::temp_dir().join(format!("viv_sub_{}", std::process::id()));

    let config = AgentConfig {
        model_tier: tier,
        max_iterations: max_iter,
        permission_mode: PermissionMode::Auto,
        memory_dir,
        ..Default::default()
    };

    let agent = crate::agent::agent::Agent::new_sub(
        config,
        endpoint,
        Arc::new(LLMClient::new(llm.config.clone())),
    )
    .await?;

    // Send the initial prompt to the sub-agent
    handle
        .tx
        .send(AgentEvent::Input(prompt.to_string()))
        .map_err(|e| Error::Tool(format!("failed to send prompt to sub-agent: {}", e)))?;

    // Run agent and monitor concurrently
    let child_future = agent.run();

    let monitor_future = async {
        let mut collected = String::new();
        loop {
            match handle.rx.try_recv() {
                Ok(AgentMessage::TextChunk(t)) => collected.push_str(&t),
                Ok(AgentMessage::Done) => {
                    // Agent finished processing — send Quit to terminate its run() loop
                    let _ = handle.tx.send(AgentEvent::Quit);
                    break;
                }
                Ok(AgentMessage::PermissionRequest { .. }) => {
                    let _ = handle.tx.send(AgentEvent::PermissionResponse(true));
                }
                Ok(_) => {} // Ignore other messages (Thinking, ToolStart, etc.)
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Yield to let the agent future make progress.
                    // In viv's single-threaded runtime (block_on_local), this
                    // allows the executor to poll other futures.
                    std::thread::yield_now();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        collected
    };

    let (agent_result, text) = join(Box::pin(child_future), Box::pin(monitor_future)).await;
    agent_result?;

    if text.is_empty() {
        Ok("Sub-agent completed without producing text output.".into())
    } else {
        Ok(text)
    }
}
