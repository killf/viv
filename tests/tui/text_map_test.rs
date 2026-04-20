use viv::tui::text_map::{CellSource, TextMap};

#[test]
fn test_text_map_set_and_get() {
    let mut map = TextMap::new();
    let source = CellSource { block: 0, span: 0, byte_offset: 5, width: 1 };
    map.set_source(10, 20, source.clone());
    assert_eq!(map.get_source(10, 20), Some(&source));
}

#[test]
fn test_text_map_get_nonexistent() {
    let map = TextMap::new();
    assert_eq!(map.get_source(0, 0), None);
}

#[test]
fn test_text_map_overwrite() {
    let mut map = TextMap::new();
    map.set_source(5, 5, CellSource { block: 0, span: 0, byte_offset: 0, width: 1 });
    map.set_source(5, 5, CellSource { block: 1, span: 2, byte_offset: 10, width: 2 });
    assert_eq!(map.get_source(5, 5), Some(&CellSource { block: 1, span: 2, byte_offset: 10, width: 2 }));
}

#[test]
fn test_text_map_clear() {
    let mut map = TextMap::new();
    map.set_source(10, 20, CellSource { block: 0, span: 0, byte_offset: 0, width: 1 });
    map.clear();
    assert!(map.is_empty());
    assert_eq!(map.get_source(10, 20), None);
}

#[test]
fn test_text_map_len() {
    let mut map = TextMap::new();
    assert_eq!(map.len(), 0);
    map.set_source(1, 1, CellSource { block: 0, span: 0, byte_offset: 0, width: 1 });
    map.set_source(2, 2, CellSource { block: 0, span: 0, byte_offset: 1, width: 1 });
    assert_eq!(map.len(), 2);
}

#[test]
fn test_text_map_multiple_cells() {
    let mut map = TextMap::new();
    for i in 0..10u16 {
        map.set_source(i, i, CellSource { block: 0, span: 0, byte_offset: i as usize, width: 1 });
    }
    assert_eq!(map.len(), 10);
    assert!(map.get_source(5, 5).is_some());
    assert!(map.get_source(9, 9).is_some());
    assert!(map.get_source(10, 10).is_none());
}

#[test]
fn test_cell_source_equality() {
    let a = CellSource { block: 1, span: 2, byte_offset: 3, width: 2 };
    let b = CellSource { block: 1, span: 2, byte_offset: 3, width: 2 };
    let c = CellSource { block: 1, span: 2, byte_offset: 4, width: 2 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}
