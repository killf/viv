use viv::memory::store::MemoryStore;
use viv::memory::index::{MemoryIndex, MemoryEntry, EntryKind};

fn tmp_store() -> MemoryStore {
    let dir = std::env::temp_dir().join(format!("viv_test_{}", std::process::id()));
    MemoryStore::new(dir).unwrap()
}

#[test]
fn store_write_and_read() {
    let store = tmp_store();
    store.write("test.txt", "hello").unwrap();
    assert_eq!(store.read("test.txt").unwrap(), "hello");
}

#[test]
fn store_exists() {
    let store = tmp_store();
    assert!(!store.exists("nope.txt"));
    store.write("yes.txt", "x").unwrap();
    assert!(store.exists("yes.txt"));
}

#[test]
fn index_save_and_load() {
    let store = tmp_store();
    let mut idx = MemoryIndex { entries: vec![] };
    idx.upsert(MemoryEntry {
        id: "e1".into(),
        kind: EntryKind::Knowledge,
        file: "knowledge/e1.md".into(),
        tags: vec!["rust".into(), "error".into()],
        summary: "Use Error enum not String".into(),
    });
    idx.save(&store).unwrap();

    let loaded = MemoryIndex::load(&store).unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].id, "e1");
    assert_eq!(loaded.entries[0].tags, vec!["rust", "error"]);
}

#[test]
fn index_keyword_search() {
    let store = tmp_store();
    let mut idx = MemoryIndex { entries: vec![] };
    idx.upsert(MemoryEntry {
        id: "k1".into(), kind: EntryKind::Knowledge,
        file: "knowledge/k1.md".into(),
        tags: vec!["rust".into()],
        summary: "zero dependency architecture".into(),
    });
    idx.upsert(MemoryEntry {
        id: "k2".into(), kind: EntryKind::Knowledge,
        file: "knowledge/k2.md".into(),
        tags: vec!["style".into()],
        summary: "use snake_case naming".into(),
    });

    let results = idx.keyword_search("rust dependency");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "k1");
}
