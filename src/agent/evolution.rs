use crate::Result;
use crate::agent::message::{Message, ContentBlock, SystemBlock};
use crate::llm::{LLMClient, ModelTier};
use crate::memory::store::MemoryStore;
use crate::memory::index::{MemoryIndex, MemoryEntry, EntryKind};
use crate::json::JsonValue;

const EVOLUTION_PROMPT: &str = r#"You just completed a conversation session. Analyze it and extract learnings.

Return a JSON array of learning objects. Each object must have:
- "kind": "fact" | "pattern" | "mistake"
- "content": string (the learning, max 2 sentences)
- "tags": array of 1-3 lowercase keyword strings
- "id": a short kebab-case identifier (e.g. "zero-deps-rule")

Example:
[{"kind":"fact","content":"User prefers zero external dependencies.","tags":["rust","deps"],"id":"zero-deps-rule"}]

Return ONLY the JSON array. If there are no significant learnings, return [].

Conversation to analyze:"#;

/// Extract learnings from a session and write them to memory.
/// Returns the number of new knowledge entries written.
pub fn evolve_from_session(
    messages: &[Message],
    store: &MemoryStore,
    index: &mut MemoryIndex,
    llm: &LLMClient,
) -> Result<usize> {
    if messages.len() < 2 {
        return Ok(0);
    }

    let conversation_text = messages_to_text(messages);
    let prompt = format!("{}\n\n{}", EVOLUTION_PROMPT, conversation_text);

    let system = vec![SystemBlock::dynamic("You are an AI learning extractor.")];
    let req_msgs = vec![Message::user_text(prompt)];
    let mut response = String::new();
    llm.stream_agent(&system, &req_msgs, "", ModelTier::Medium, |t| response.push_str(t))?;

    let learnings = parse_learnings(&response);
    let count = learnings.len();

    for learning in learnings {
        let file = format!("knowledge/{}.md", learning.id);
        let content = format!(
            "---\nkind: {}\ntags: {}\n---\n\n{}\n",
            learning.kind,
            learning.tags.join(", "),
            learning.content,
        );
        store.write(&file, &content)?;
        index.upsert(MemoryEntry {
            id: learning.id.clone(),
            kind: EntryKind::Knowledge,
            file,
            tags: learning.tags,
            summary: learning.content,
        });
    }
    index.save(store)?;

    save_episode(messages, store, index, llm)?;

    Ok(count)
}

struct Learning {
    id: String,
    kind: String,
    content: String,
    tags: Vec<String>,
}

fn parse_learnings(response: &str) -> Vec<Learning> {
    let start = response.find('[').unwrap_or(0);
    let end = response.rfind(']').map(|i| i + 1).unwrap_or(response.len());
    let json_str = &response[start..end];

    let json = match JsonValue::parse(json_str) { Ok(j) => j, Err(_) => return vec![] };
    let arr = match json.as_array() { Some(a) => a, None => return vec![] };

    arr.iter().filter_map(|item| {
        let id = item.get("id")?.as_str()?.to_string();
        let kind = item.get("kind")?.as_str()?.to_string();
        let content = item.get("content")?.as_str()?.to_string();
        let tags = item.get("tags")?.as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        if id.is_empty() || content.is_empty() { return None; }
        Some(Learning { id, kind, content, tags })
    }).collect()
}

fn save_episode(
    messages: &[Message],
    store: &MemoryStore,
    index: &mut MemoryIndex,
    llm: &LLMClient,
) -> Result<()> {
    let summary_prompt = format!(
        "Summarize this conversation in 1-2 sentences, focusing on what was accomplished:\n\n{}",
        messages_to_text(messages),
    );
    let system = vec![SystemBlock::dynamic("You are a conversation summarizer.")];
    let req_msgs = vec![Message::user_text(summary_prompt)];
    let mut summary = String::new();
    llm.stream_agent(&system, &req_msgs, "", ModelTier::Fast, |t| summary.push_str(t))?;
    let summary = summary.trim().to_string();

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("ep-{}", ts);
    let file = format!("episodes/{}.md", id);
    let tags = extract_tags_from_summary(&summary);

    store.write(&file, &format!("# Episode\n\n{}\n", summary))?;
    index.upsert(MemoryEntry {
        id: id.clone(),
        kind: EntryKind::Episode,
        file,
        tags,
        summary,
    });
    index.save(store)
}

fn extract_tags_from_summary(summary: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "the","a","an","in","on","at","to","for","of","and","or","with",
        "this","that","was","were","is","are","it","its","been","have",
        "has","had","from","by","as","be","not","but",
    ];
    summary.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 4)
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .map(|w| w.to_lowercase())
        .take(3)
        .collect()
}

fn messages_to_text(messages: &[Message]) -> String {
    messages.iter().map(|m| {
        let role = m.role();
        let text = m.blocks().iter().filter_map(|b| {
            if let ContentBlock::Text(t) = b { Some(t.as_str()) } else { None }
        }).collect::<Vec<_>>().join(" ");
        format!("{}: {}", role, text)
    }).collect::<Vec<_>>().join("\n")
}
