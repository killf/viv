use crate::json::JsonValue;
use crate::Result;
use super::store::MemoryStore;

const INDEX_FILE: &str = "index.json";

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: EntryKind,
    pub file: String,
    pub tags: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Episode,
    Knowledge,
}

pub struct MemoryIndex {
    pub entries: Vec<MemoryEntry>,
}

impl MemoryIndex {
    pub fn load(store: &MemoryStore) -> Result<Self> {
        if !store.exists(INDEX_FILE) {
            return Ok(MemoryIndex { entries: vec![] });
        }
        let raw = store.read(INDEX_FILE)?;
        let json = JsonValue::parse(&raw).unwrap_or(JsonValue::Object(vec![]));
        let mut entries = vec![];

        for kind_key in &["episodes", "knowledge"] {
            let kind = if *kind_key == "episodes" { EntryKind::Episode } else { EntryKind::Knowledge };
            if let Some(arr) = json.get(kind_key).and_then(|v| v.as_array()) {
                for item in arr {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let tags = item.get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    entries.push(MemoryEntry { id, kind: kind.clone(), file, tags, summary });
                }
            }
        }
        Ok(MemoryIndex { entries })
    }

    pub fn save(&self, store: &MemoryStore) -> Result<()> {
        let episodes: Vec<&MemoryEntry> = self.entries.iter().filter(|e| e.kind == EntryKind::Episode).collect();
        let knowledge: Vec<&MemoryEntry> = self.entries.iter().filter(|e| e.kind == EntryKind::Knowledge).collect();

        fn entry_json(e: &MemoryEntry) -> String {
            let tags: Vec<String> = e.tags.iter().map(|t| format!("\"{}\"", t)).collect();
            format!(
                "{{\"id\":\"{}\",\"file\":\"{}\",\"tags\":[{}],\"summary\":\"{}\"}}",
                e.id, e.file, tags.join(","),
                e.summary.replace('"', "\\\""),
            )
        }

        let ep_json: Vec<String> = episodes.iter().map(|e| entry_json(e)).collect();
        let kn_json: Vec<String> = knowledge.iter().map(|e| entry_json(e)).collect();

        let content = format!(
            "{{\"version\":1,\"episodes\":[{}],\"knowledge\":[{}]}}",
            ep_json.join(","),
            kn_json.join(","),
        );
        store.write(INDEX_FILE, &content)
    }

    /// 关键词预筛：返回 summary 或 tags 中包含 query 词的条目
    pub fn keyword_search(&self, query: &str) -> Vec<&MemoryEntry> {
        let words: Vec<&str> = query.split_whitespace().collect();
        self.entries.iter().filter(|e| {
            words.iter().any(|w| {
                let w_lower = w.to_lowercase();
                e.summary.to_lowercase().contains(&w_lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&w_lower))
            })
        }).collect()
    }

    pub fn upsert(&mut self, entry: MemoryEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}
