//! TextMap: maps screen coordinates to text content sources.
//! Built during rendering, used during Ctrl+C copy.

use std::collections::HashMap;

/// A source reference: which block/span/byte-offset a screen cell came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSource {
    /// Index into the `blocks` array.
    pub block: usize,
    /// Index into that block's spans (for Markdown blocks).
    pub span: usize,
    /// UTF-8 byte offset within that span's text.
    pub byte_offset: usize,
    /// Display width of this character (1 or 2 for CJK).
    pub width: u16,
}

/// Maps screen cell coordinates to the text source they were rendered from.
#[derive(Debug, Clone, Default)]
pub struct TextMap {
    cells: HashMap<(u16, u16), CellSource>,
}

impl TextMap {
    /// Create an empty TextMap.
    pub fn new() -> Self {
        TextMap { cells: HashMap::new() }
    }

    /// Record that screen cell (x, y) was rendered from the given source.
    pub fn set_source(&mut self, x: u16, y: u16, source: CellSource) {
        self.cells.insert((x, y), source);
    }

    /// Look up the source for screen cell (x, y).
    pub fn get_source(&self, x: u16, y: u16) -> Option<&CellSource> {
        self.cells.get(&(x, y))
    }

    /// Clear all mappings (called at the start of each frame).
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Returns the number of mapped cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Iterate over all mapped cells.
    pub fn cells(&self) -> &HashMap<(u16, u16), CellSource> {
        &self.cells
    }
}
