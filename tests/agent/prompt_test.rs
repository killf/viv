use viv::agent::message::PromptCache;
use viv::agent::prompt::build_system_prompt;

#[test]
fn prompt_has_base_block_with_cache() {
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("/test", "", "", &[], &mut cache);
    assert!(!sp.blocks.is_empty());
    assert!(sp.blocks[0].cached);
}

#[test]
fn prompt_env_block_added() {
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("/test", "", "", &[], &mut cache);
    // Base + Env = 2 blocks minimum
    assert!(sp.blocks.len() >= 2);
    // Env block should be cached
    assert!(sp.blocks[1].cached);
    // Should contain cwd
    assert!(sp.blocks[1].text.contains("/test"));
}

#[test]
fn prompt_tools_block_added_when_nonempty() {
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("/test", "tool: bash", "", &[], &mut cache);
    // Base + Env + Tools = 3 blocks
    assert_eq!(sp.blocks.len(), 3);
    assert!(sp.blocks[2].cached);
}

#[test]
fn prompt_cache_reuses_text_on_same_hash() {
    let mut cache = PromptCache::default();
    let _ = build_system_prompt("/test", "tools v1", "", &[], &mut cache);
    let h1 = cache.tools_hash;
    let _ = build_system_prompt("/test", "tools v1", "", &[], &mut cache);
    assert_eq!(cache.tools_hash, h1);
}

#[test]
fn prompt_cache_updates_on_changed_content() {
    let mut cache = PromptCache::default();
    let _ = build_system_prompt("/test", "tools v1", "", &[], &mut cache);
    let h1 = cache.tools_hash;
    let _ = build_system_prompt("/test", "tools v2", "", &[], &mut cache);
    assert_ne!(cache.tools_hash, h1);
}

#[test]
fn memory_block_not_cached() {
    use viv::memory::index::{EntryKind, MemoryEntry};
    use viv::memory::retrieval::RetrievalResult;
    let results = vec![RetrievalResult {
        entry: MemoryEntry {
            id: "1".into(),
            kind: EntryKind::Knowledge,
            file: "f.md".into(),
            tags: vec![],
            summary: "test fact".into(),
        },
        content: "test fact".into(),
    }];
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("/test", "", "", &results, &mut cache);
    let last = sp.blocks.last().unwrap();
    assert!(!last.cached);
}
