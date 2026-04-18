use crate::Result;
use crate::agent::message::{ContentBlock, Message, SystemBlock};
use crate::llm::{LLMClient, ModelTier};

/// Compact old messages into a summary when approaching token limit.
/// Keeps the most recent `keep_recent` turn pairs, summarizes the rest.
pub async fn compact_if_needed(
    messages: &mut Vec<Message>,
    token_estimate: usize,
    token_limit: usize,
    keep_recent: usize,
    llm: &LLMClient,
) -> Result<()> {
    if token_estimate < token_limit * 8 / 10 {
        return Ok(());
    }
    if messages.len() <= keep_recent * 2 {
        return Ok(());
    }

    let split_at = messages.len().saturating_sub(keep_recent * 2);
    let to_compress: Vec<&Message> = messages[..split_at].iter().collect();

    let summary_prompt = format!(
        "Summarize this conversation history concisely (2-4 sentences):\n\n{}",
        messages_to_text(&to_compress),
    );
    let system = vec![SystemBlock::dynamic("You are a conversation summarizer.")];
    let req_msgs = vec![Message::user_text(summary_prompt)];
    let mut summary = String::new();
    llm.stream_agent_async(&system, &req_msgs, "", ModelTier::Fast, |t| {
        summary.push_str(t)
    })
    .await?;

    let recent = messages.split_off(split_at);
    messages.clear();
    messages.push(Message::User(vec![ContentBlock::Text(format!(
        "[Earlier conversation summary]\n{}",
        summary
    ))]));
    messages.extend(recent);

    Ok(())
}

fn messages_to_text(messages: &[&Message]) -> String {
    messages
        .iter()
        .map(|m| {
            let role = m.role();
            let text = m
                .blocks()
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text(t) = b {
                        Some(t.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}: {}", role, text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rough token estimate: characters / 4
pub fn estimate_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| {
            m.blocks()
                .iter()
                .map(|b| match b {
                    ContentBlock::Text(t) => t.len() / 4,
                    ContentBlock::ToolUse { input, .. } => input.to_string().len() / 4,
                    ContentBlock::ToolResult { content, .. } => content
                        .iter()
                        .map(|c| {
                            if let ContentBlock::Text(t) = c {
                                t.len() / 4
                            } else {
                                10
                            }
                        })
                        .sum::<usize>(),
                })
                .sum::<usize>()
        })
        .sum()
}
