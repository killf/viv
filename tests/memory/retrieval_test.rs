use viv::memory::retrieval::{format_memory_injection, RetrievalResult};
use viv::memory::index::{MemoryEntry, EntryKind};

fn make_result(summary: &str, kind: EntryKind) -> RetrievalResult {
    RetrievalResult {
        entry: MemoryEntry {
            id: "x".into(), kind, file: "f.md".into(),
            tags: vec![], summary: summary.into(),
        },
        content: summary.into(),
    }
}

#[test]
fn format_empty_returns_empty() {
    assert!(format_memory_injection(&[]).is_empty());
}

#[test]
fn format_includes_summaries() {
    let results = vec![
        make_result("zero dependency", EntryKind::Knowledge),
        make_result("session 2026-04-10", EntryKind::Episode),
    ];
    let out = format_memory_injection(&results);
    assert!(out.contains("<memory>"));
    assert!(out.contains("[Knowledge] zero dependency"));
    assert!(out.contains("[Episodic] session 2026-04-10"));
    assert!(out.contains("</memory>"));
}
