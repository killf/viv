use super::index::{MemoryEntry, MemoryIndex};
use super::store::MemoryStore;
use crate::Result;
use crate::llm::{LLMClient, ModelTier};

pub struct RetrievalResult {
    pub entry: MemoryEntry,
    pub content: String,
}

/// Two-stage retrieval: keyword pre-filter → LLM relevance ranking → Top-K
pub async fn retrieve_relevant(
    query: &str,
    index: &MemoryIndex,
    store: &MemoryStore,
    llm: &LLMClient,
    top_k: usize,
) -> Result<Vec<RetrievalResult>> {
    // Stage 1: keyword pre-filter (max 20 candidates)
    let mut candidates = index.keyword_search(query);
    candidates.truncate(20);

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // Stage 2: LLM relevance ranking (only when candidates > top_k)
    let selected = if candidates.len() <= top_k {
        candidates
    } else {
        llm_rank(query, candidates, llm, top_k).await?
    };

    // Read file contents
    let mut results = vec![];
    for entry in selected {
        if let Ok(content) = store.read(&entry.file) {
            results.push(RetrievalResult {
                entry: entry.clone(),
                content,
            });
        }
    }
    Ok(results)
}

async fn llm_rank<'a>(
    query: &str,
    candidates: Vec<&'a MemoryEntry>,
    llm: &LLMClient,
    top_k: usize,
) -> Result<Vec<&'a MemoryEntry>> {
    let list: Vec<String> = candidates
        .iter()
        .enumerate()
        .map(|(i, e)| format!("[{}] {}", i, e.summary))
        .collect();

    let prompt = format!(
        "Task: \"{}\"\n\nMemory candidates:\n{}\n\nReturn the indices of the {} most relevant memories as JSON array, e.g. [0,2,3]. Return ONLY the JSON array, nothing else.",
        query,
        list.join("\n"),
        top_k,
    );

    use crate::agent::message::{Message, SystemBlock};
    let system = vec![SystemBlock::dynamic(
        "You are a memory retrieval assistant.",
    )];
    let messages = vec![Message::user_text(prompt)];
    let mut response = String::new();
    llm.stream_agent_async(&system, &messages, "", ModelTier::Fast, |t| {
        response.push_str(t)
    })
    .await?;

    let indices = parse_index_array(&response);
    Ok(indices
        .into_iter()
        .filter(|&i| i < candidates.len())
        .map(|i| candidates[i])
        .take(top_k)
        .collect())
}

fn parse_index_array(s: &str) -> Vec<usize> {
    let start = s.find('[').unwrap_or(0);
    let end = s.rfind(']').map(|i| i + 1).unwrap_or(s.len());
    let slice = &s[start..end];
    slice
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect()
}

/// Format retrieval results for injection into system prompt
pub fn format_memory_injection(results: &[RetrievalResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut out = String::from("<memory>\n");
    for r in results {
        let kind = match r.entry.kind {
            super::index::EntryKind::Episode => "Episodic",
            super::index::EntryKind::Knowledge => "Knowledge",
        };
        out.push_str(&format!("[{}] {}\n", kind, r.entry.summary));
    }
    out.push_str("</memory>");
    out
}
