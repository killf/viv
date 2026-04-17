use crate::Result;
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
    tool_descriptions: &str,
    skill_contents: &str,
    mut on_text: impl FnMut(&str),
) -> Result<AgentOutput> {
    // 1. Retrieve relevant memories
    let memories = {
        let idx = ctx.index.lock().unwrap();
        retrieve_relevant(&input, &idx, &ctx.store, &ctx.llm, ctx.config.top_k_memory)?
    };

    // 2. Build system prompt (cache-first)
    let system = build_system_prompt(tool_descriptions, skill_contents, &memories, &mut ctx.prompt_cache);

    // 3. Append user message
    ctx.messages.push(Message::user_text(input));

    // 4. Compact context if needed
    let token_estimate = estimate_tokens(&ctx.messages);
    compact_if_needed(&mut ctx.messages, token_estimate, 100_000, 10, &ctx.llm)?;

    let mut final_text = String::new();
    let mut iterations = 0;

    loop {
        if iterations >= ctx.config.max_iterations { break; }
        iterations += 1;

        // 5. Call LLM
        let stream_result = ctx.llm.stream_agent(
            &system.blocks,
            &ctx.messages,
            "",
            ctx.config.model_tier.clone(),
            &mut on_text,
        )?;

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

        // 8. Execute tools (stub: returns "tool not yet implemented")
        let tool_results: Vec<ContentBlock> = stream_result.tool_uses.iter().map(|tu| {
            if let ContentBlock::ToolUse { id, .. } = tu {
                ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: vec![ContentBlock::Text("Tool not yet implemented.".into())],
                    is_error: false,
                }
            } else { unreachable!() }
        }).collect();

        // 9. Append tool results as user message (Anthropic API requirement)
        ctx.messages.push(Message::User(tool_results));
    }

    Ok(AgentOutput { text: final_text, iterations })
}
