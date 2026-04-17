use crate::Result;
use crate::core::json::JsonValue;
use crate::agent::context::AgentContext;
use crate::agent::message::{Message, ContentBlock};
use crate::agent::prompt::build_system_prompt;
use crate::memory::retrieval::retrieve_relevant;
use crate::memory::compaction::{compact_if_needed, estimate_tokens};

pub struct AgentOutput {
    pub text: String,
    pub iterations: usize,
}

pub fn run_agent(
    input: String,
    ctx: &mut AgentContext,
    ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    mut on_text: impl FnMut(&str),
) -> Result<AgentOutput> {
    // 1. Retrieve relevant memories
    let memories = {
        let idx = ctx.index.lock().unwrap();
        retrieve_relevant(&input, &idx, &ctx.store, &ctx.llm, ctx.config.top_k_memory)?
    };

    // 2. Build system prompt (cache-first)
    let system = build_system_prompt("", "", &memories, &mut ctx.prompt_cache);

    // 3. Append user message
    ctx.messages.push(Message::user_text(input));

    // 4. Compact context if needed
    let token_estimate = estimate_tokens(&ctx.messages);
    compact_if_needed(&mut ctx.messages, token_estimate, 100_000, 10, &ctx.llm)?;

    let mut final_text = String::new();
    let mut iterations = 0;

    let tools_json = ctx.tool_registry.to_api_json();

    loop {
        if iterations >= ctx.config.max_iterations { break; }
        iterations += 1;

        // 5. Call LLM
        let stream_result = ctx.llm.stream_agent(
            &system.blocks,
            &ctx.messages,
            &tools_json,
            ctx.config.model_tier.clone(),
            &mut on_text,
        )?;

        ctx.input_tokens += stream_result.input_tokens;
        ctx.output_tokens += stream_result.output_tokens;

        // 6. Collect assistant response blocks
        let mut assistant_blocks: Vec<ContentBlock> = stream_result.text_blocks.clone();
        assistant_blocks.extend(stream_result.tool_uses.clone());

        for b in &stream_result.text_blocks {
            if let ContentBlock::Text(t) = b { final_text = t.clone(); }
        }

        ctx.messages.push(Message::Assistant(assistant_blocks));

        // 7. No tool calls → done
        if stream_result.tool_uses.is_empty() || stream_result.stop_reason == "end_turn" {
            break;
        }

        // 8. Execute tools with permission gating
        let mut tool_results: Vec<ContentBlock> = Vec::new();
        for tu in &stream_result.tool_uses {
            if let ContentBlock::ToolUse { id, name, input } = tu {
                let result = match ctx.tool_registry.get(name) {
                    None => Err(crate::Error::Tool(format!("unknown tool: {}", name))),
                    Some(tool) => {
                        // tool borrows ctx.tool_registry (immutably);
                        // ctx.permission_manager is a separate field — Rust NLL allows this split borrow
                        if ctx.permission_manager.check(tool, input, ask_fn) {
                            tool.execute(input)
                        } else {
                            Err(crate::Error::Tool("permission denied by user".into()))
                        }
                    }
                };
                let (content, is_error) = match result {
                    Ok(text) => (text, false),
                    Err(e) => (e.to_string(), true),
                };
                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: vec![ContentBlock::Text(content)],
                    is_error,
                });
            }
        }

        // 9. Append tool results as user message (Anthropic API requirement)
        ctx.messages.push(Message::User(tool_results));
    }

    Ok(AgentOutput { text: final_text, iterations })
}
