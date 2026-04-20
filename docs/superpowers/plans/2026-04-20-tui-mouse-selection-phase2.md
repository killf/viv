# TUI 鼠标选择与复制 - Phase 2 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 TextMap 坐标映射，从 blocks 中正确提取选中文本，Ctrl+C 复制实际文本内容

**Architecture:**
- 新增 `TextMap` 在渲染时构建屏幕坐标到文本内容的映射
- `CellSource` 记录每个单元格对应的 (block_idx, span_idx, byte_offset)
- `extract_selection_text()` 根据 TextMap 和选择区域提取文本
- Ctrl+C 改为调用 `extract_selection_text()` 并打印结果

**Tech Stack:**
- 稀疏 HashMap 存储映射关系
- UTF-8 字节偏移量处理 CJK 多字节字符
- Phase 1 的 `SelectionState.region()` 提供坐标范围

---

## 文件结构

**新建文件：**
- `src/tui/text_map.rs` - TextMap 和 CellSource 定义
- `tests/tui/text_map_test.rs` - 单元测试

**修改文件：**
- `src/tui/renderer.rs` - render() 时调用 text_map 构建
- `src/tui/terminal.rs:592-600` - Ctrl+C 打印实际文本而非坐标

---

## Task 1: TextMap 和 CellSource

**Files:**
- Create: `src/tui/text_map.rs`
- Create: `tests/tui/text_map_test.rs`
- Modify: `src/tui/mod.rs` — 添加 `pub mod text_map;`

- [ ] **Step 1: 创建 text_map.rs**

```rust
// src/tui/text_map.rs

use std::collections::HashMap;

/// 来源：一个屏幕单元格对应的 blocks 内容位置
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSource {
    /// blocks 数组中的索引
    pub block: usize,
    /// 该 block 中 span 的索引
    pub span: usize,
    /// 该 span 中的字节偏移量（UTF-8）
    pub byte_offset: usize,
    /// 该字符的显示宽度（1 或 2，用于 CJK）
    pub width: u16,
}

/// 屏幕坐标到文本内容的映射
/// 在 Widget::render() 时构建，用于 Ctrl+C 复制选中文本
#[derive(Debug, Clone)]
pub struct TextMap {
    cells: HashMap<(u16, u16), CellSource>,
}

impl TextMap {
    pub fn new() -> Self {
        TextMap {
            cells: HashMap::new(),
        }
    }

    /// 记录一个屏幕单元格对应的文本来源
    pub fn set_source(&mut self, x: u16, y: u16, source: CellSource) {
        self.cells.insert((x, y), source);
    }

    /// 获取一个屏幕单元格对应的文本来源
    pub fn get_source(&self, x: u16, y: u16) -> Option<&CellSource> {
        self.cells.get(&(x, y))
    }

    /// 清除所有映射（每帧重新构建）
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// 获取所有映射（用于提取）
    pub fn cells(&self) -> &HashMap<(u16, u16), CellSource> {
        &self.cells
    }
}

impl Default for TextMap {
    fn default() -> Self {
        Self::new()
    }
}
```

Run: `cargo build`

- [ ] **Step 2: 添加到 mod.rs**

```rust
// src/tui/mod.rs
pub mod text_map;
```

Run: `cargo build`

- [ ] **Step 3: 添加测试**

```rust
// tests/tui/text_map_test.rs

use viv::tui::text_map::{CellSource, TextMap};

#[test]
fn test_text_map_set_and_get() {
    let mut map = TextMap::new();
    let source = CellSource { block: 0, span: 0, byte_offset: 5, width: 1 };
    map.set_source(10, 20, source.clone());
    assert_eq!(map.get_source(10, 20), Some(&source));
}

#[test]
fn test_text_map_get_none() {
    let map = TextMap::new();
    assert_eq!(map.get_source(0, 0), None);
}

#[test]
fn test_text_map_clear() {
    let mut map = TextMap::new();
    map.set_source(10, 20, CellSource { block: 0, span: 0, byte_offset: 0, width: 1 });
    map.clear();
    assert_eq!(map.get_source(10, 20), None);
}

#[test]
fn test_cell_source_clone() {
    let source = CellSource { block: 1, span: 2, byte_offset: 10, width: 2 };
    let cloned = source.clone();
    assert_eq!(source, cloned);
}
```

Run: `cargo test test_text_map`

- [ ] **Step 4: Commit**

```bash
git add src/tui/text_map.rs src/tui/mod.rs tests/tui/text_map_test.rs
git commit -m "feat(tui): add TextMap for coordinate-to-text mapping"
```

---

## Task 2: Widget 渲染时构建 TextMap

**Files:**
- Modify: `src/tui/renderer.rs` — Renderer 添加 text_map 字段
- Modify: `src/tui/terminal.rs` — 传递 text_map 给 renderers

- [ ] **Step 1: 在 Renderer 添加 text_map 字段**

```rust
// src/tui/renderer.rs

use crate::core::terminal::backend::Backend;
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::size::TermSize;
use crate::tui::text_map::TextMap;  // 新增

pub struct Renderer {
    current: Buffer,
    previous: Buffer,
    size: TermSize,
    last_cursor: Option<(u16, u16)>,
    pub(crate) selection: Option<Rect>,
    pub text_map: TextMap,  // 新增：每帧重新构建的坐标映射
}
```

在 `Renderer::new()` 中初始化：
```rust
Renderer {
    current: Buffer::empty(area),
    previous: Buffer::empty(area),
    size,
    last_cursor: None,
    selection: None,
    text_map: TextMap::new(),  // 新增
}
```

在 `Renderer::resize()` 中清除：
```rust
self.selection = None;
self.text_map.clear();  // 新增
```

Run: `cargo build`

- [ ] **Step 2: 添加 buffer_mut 方法接受 text_map**

在 `Renderer` 中添加：
```rust
/// Returns the current buffer and text_map for widgets to render into.
pub fn buffer_map_mut(&mut self) -> (&mut Buffer, &mut TextMap) {
    self.text_map.clear();  // 每帧开始时清除，重新构建
    (&mut self.current, &mut self.text_map)
}
```

- [ ] **Step 3: 在 TerminalUI 中使用 buffer_map_mut**

找到 TerminalUI 中调用 `self.renderer.buffer_mut()` 的地方，改为调用 `self.renderer.buffer_map_mut()`。

需要先找到哪里调用了 buffer_mut。搜索：
```bash
grep -n "buffer_mut" src/tui/terminal.rs
```

将调用点改为：
```rust
let (buf, text_map) = self.renderer.buffer_map_mut();
// 然后将 buf 和 text_map 传递给需要渲染的函数
```

注意：需要修改渲染链上的函数签名来传递 text_map。需要找到所有使用 `renderer.buffer_mut()` 的地方并修改。

- [ ] **Step 4: 查找 buffer_mut 调用点**

```bash
grep -n "buffer_mut\|render\|buf" src/tui/terminal.rs | head -30
```

需要查看完整的渲染循环来理解 buf/text_map 的流向。先读取 terminal.rs 中渲染相关的部分。

找到渲染循环中的 `self.renderer.buffer_mut()` 并替换为 `self.renderer.buffer_map_mut()`。同时需要找到所有使用 buf 的地方。

- [ ] **Step 5: 读取 terminal.rs 渲染部分并修改**

这是最复杂的部分。需要修改整个渲染调用链来传递 text_map。

步骤：
1. 在 terminal.rs 中用 `buffer_map_mut()` 替换 `buffer_mut()`
2. 修改所有 render 调用，增加 `text_map` 参数
3. 修改各个 Widget 的 render 方法签名（需要先找到哪些 Widget 被调用）

先读取 terminal.rs 的渲染部分来了解结构：
```bash
grep -n "fn render\|buffer_mut\|widget\|Widget" src/tui/terminal.rs | head -30
```

- [ ] **Step 6: 编译并修复错误**

`cargo build` 会报很多错，因为 Widget::render 签名没变。先做最小化修改：

方案：直接在 TerminalUI 的渲染循环末尾调用一个方法来构建 TextMap，而不是修改每个 Widget 的签名。

具体做法：
1. 保持 `buffer_mut()` 不变（不修改 Widget::render 签名）
2. 在 TerminalUI 中，渲染完成后，有一个最终的位置知道每个 block 被渲染在哪里
3. 但这需要每个 Widget 记录自己渲染的位置和内容

实际上最简单的方案还是：修改 buffer_mut → buffer_map_mut，然后修改所有调用链。但这很复杂。

替代方案（更实用）：
1. 在 Renderer 中保存 `text_map: TextMap`
2. 添加 `renderer.push_cell_source(x, y, source)` 方法
3. 在 TerminalUI 渲染循环中，每渲染一个内容块时手动调用 `push_cell_source`
4. 这样不需要修改 Widget::render 签名

**重新设计 Step 3-6：**

- [ ] **Step 3 (修订): 在 Renderer 添加简单 API**

```rust
// src/tui/renderer.rs

use crate::tui::text_map::{CellSource, TextMap};

pub struct Renderer {
    current: Buffer,
    previous: Buffer,
    size: TermSize,
    last_cursor: Option<(u16, u16)>,
    pub(crate) selection: Option<Rect>,
    text_map: TextMap,
}

impl Renderer {
    // 现有方法不变...

    /// 在渲染过程中记录单元格的文本来源
    /// 由 TerminalUI 在渲染每个内容块时调用
    pub fn map_cell(&mut self, x: u16, y: u16, source: CellSource) {
        self.text_map.set_source(x, y, source);
    }

    /// 清除 text_map（每帧开始时调用）
    pub fn clear_text_map(&mut self) {
        self.text_map.clear();
    }

    /// 获取 text_map（用于文本提取）
    pub fn text_map(&self) -> &TextMap {
        &self.text_map
    }
}

impl Default for TextMap {
    fn default() -> Self {
        Self::new()
    }
}
```

在 `Renderer::resize()` 中：
```rust
self.text_map.clear();
```

在 `Renderer::flush()` 开始时（如果每帧重建）：
flush() 开头已经有 `self.current.clear()`，在那里加上 `self.text_map.clear()` 即可。

实际上 flush() 的末尾已经有 `self.current.clear()`，但 text_map 应该在渲染开始时清除，而不是 flush 时。

在 TerminalUI 渲染循环中，找到开始渲染的地方，在 buffer_mut() 调用前加上：
```rust
self.renderer.clear_text_map();
```

- [ ] **Step 4 (修订): TerminalUI 渲染循环添加 clear_text_map**

在 terminal.rs 中找到渲染循环，在 `let buf = self.renderer.buffer_mut();` 之前加上：
```rust
self.renderer.clear_text_map();
```

- [ ] **Step 5 (修订): TerminalUI 渲染循环中手动构建 TextMap**

这是关键部分。需要在 TerminalUI 渲染每个内容块时，调用 `renderer.map_cell()` 来记录映射关系。

找到渲染 blocks 的循环。需要在这里遍历每个 block 的内容，并调用 map_cell。

方案：
1. 创建一个辅助函数 `render_block_with_map` 来渲染单个 block 并构建映射
2. 在 TerminalUI 渲染循环中调用这个辅助函数

或者更简单：在 TerminalUI 中，在渲染每个 block 后，重新遍历该 block 的文本内容并填充 TextMap（类似于 Widget 自己的 render，但只做映射）。

具体实现：
```rust
// 在 TerminalUI 中，在渲染 blocks 之后，flush 之前，补充构建 TextMap
// 调用 extract_spans_from_block(block) 来获取该 block 的所有文本片段
// 然后根据 block 的渲染位置，计算每个字符对应的屏幕坐标
// 调用 renderer.map_cell(x, y, CellSource { block, span, byte_offset, width })
```

这需要知道每个 block 被渲染在哪个区域。检查 ConversationState 是否有这个信息。

实际上，更实用的方案：在各个 Widget 的 render 方法被调用时，它们已经知道自己的 area 参数。可以在 TerminalUI 渲染循环中，调用 Widget::render 之后，直接根据 block 的内容重新计算并填充 TextMap。

**简化方案（不需要修改 Widget::render 签名）：**

在 TerminalUI 的渲染循环中，渲染完所有内容后，遍历 blocks 并手动构建 TextMap：

```rust
// 在 flush() 之前，补充 TextMap
self.build_text_map();
```

`build_text_map` 方法：
```rust
fn build_text_map(&mut self) {
    let buf = self.renderer.buffer_map_mut();  // 或者添加一个 text_map_mut()
    let (mut buf, text_map) = buf;
    let area = self.renderer.area();
    
    for (block_idx, block) in self.blocks.iter().enumerate() {
        // 获取该 block 的可见范围
        let visible = self.conversation_state.visible_items();
        if let Some(item) = visible.get(block_idx) {
            let block_y = item.viewport_y - self.conversation_state.scroll_offset;
            
            // 遍历 block 的内容并填充 text_map
            self.map_block_content(block, block_idx, item.clip_top, block_y, text_map);
        }
    }
}
```

等等，这太复杂了。更简单的方案：

**终极简化方案**：直接在 TerminalUI 中实现 `build_text_map`，不修改任何 Widget::render 签名。

`build_text_map` 需要：
1. 遍历所有 blocks
2. 对每个 block，获取其渲染的文本内容
3. 根据 block 的屏幕位置，计算每个字符的屏幕坐标
4. 调用 `text_map.set_source(x, y, CellSource {...})`

但这需要知道每个 block 的渲染位置（viewport_y）。ConversationState 有这些信息。

让我看看 ConversationState 的可见项接口：
```bash
grep -n "visible\|item_height\|viewport\|clip" src/tui/conversation.rs | head -20
```

根据之前读取的代码，ConversationState 有 `item_heights` 和 `scroll_offset`，但没有直接的 visible_items API。需要计算每个 block 的渲染位置。

- [ ] **Step 6: 实现 build_text_map**

在 TerminalUI 添加：

```rust
fn build_text_map(&mut self, text_map: &mut TextMap) {
    // 遍历所有 blocks，根据渲染位置填充 TextMap
    let mut y: u16 = 0;
    for (block_idx, block) in self.blocks.iter().enumerate() {
        let block_height = self.conversation_state.item_heights
            .get(block_idx)
            .copied()
            .unwrap_or(1);
        
        // 渲染位置
        let viewport_y = y.saturating_sub(self.conversation_state.scroll_offset);
        
        // 提取该 block 的文本内容并填充 text_map
        self.map_block_to_text_map(block, block_idx, viewport_y, text_map);
        
        y += block_height;
    }
}

fn map_block_to_text_map(&self, block: &ContentBlock, block_idx: usize, start_y: u16, text_map: &mut TextMap) {
    match block {
        ContentBlock::Markdown { nodes } => {
            // 遍历所有 MarkdownNode
            for node in nodes {
                match node {
                    MarkdownNode::Paragraph { spans } => {
                        let mut line_y = start_y;
                        let mut col: u16 = 0;
                        for span in spans {
                            for ch in span.text.chars() {
                                let width = char_width(ch) as u16;
                                // 设置映射（这里简化处理，实际需要更精确的行列计算）
                                let source = CellSource {
                                    block: block_idx,
                                    span: 0,  // 需要更精确的 span 索引
                                    byte_offset: 0,  // 需要计算
                                    width,
                                };
                                // 需要将 col, line_y 映射到实际屏幕坐标
                                // 但这需要知道该 block 渲染在哪个 x 偏移量
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // 处理其他 block 类型...
        _ => {}
    }
}
```

**问题**：这个方法太复杂了，因为需要知道每个 block 渲染在屏幕上的精确位置（x 偏移量）。

**最佳方案（重新考虑）**：最正确的做法是在 Widget 的 render 方法中构建映射，因为 Widget 知道自己的 area 参数。

**最终决定**：修改 Renderer::buffer_mut → buffer_map_mut，返回 (&mut Buffer, &mut TextMap)，然后修改所有调用链上的函数签名传递 TextMap。

这确实是最干净的方案，但需要大量修改。

**折中方案**：
1. Renderer 添加 `text_map: TextMap` 字段
2. Renderer 添加 `fn text_map_mut(&mut self) -> &mut TextMap`
3. TerminalUI 中，在渲染前 `renderer.clear_text_map()`
4. TerminalUI 中，在渲染后、flush 前，调用一个 `rebuild_text_map_from_blocks()` 方法
5. `rebuild_text_map_from_blocks()` 只遍历 blocks 中的文本，计算屏幕坐标，填充 TextMap

`rebuild_text_map_from_blocks()` 不需要知道精确的渲染区域 —— 它根据以下信息计算：
- block_idx → 该 block 的高度（来自 conversation_state）
- 每个 block 的渲染起点 y 坐标（根据 scroll_offset 计算可见区域的起点）
- 每个 block 内每个字符的 x 坐标（根据当前宽度 word wrap 计算）

这需要知道终端宽度和 word wrap 逻辑。Paragraph widget 的 word wrap 逻辑是关键。

**再次简化**：最实用的做法是在 Widget 的 render 方法末尾调用 `renderer.map_cell()`。由于 Widget 已经有 `area: Rect` 参数，它们知道自己的渲染位置。

- 修改 Widget::render 签名为 `fn render(self, area: Rect, buf: &mut Buffer, text_map: &mut TextMap)`
- TerminalUI 传递 text_map 给 render 调用

这是正确的方案。让我执行这个。

---

实际上，我意识到一个关键问题：**每个字符的 UTF-8 字节偏移量**需要精确计算。如果在 Widget render 时计算，每个 Widget 都要修改。

**最简单可行的方案**：
1. Renderer 添加 `text_map: TextMap` 和 `text_map_mut()`
2. 在 TerminalUI 中，渲染循环中，每个 block 渲染后，立即计算该 block 的 TextMap
3. `build_text_map_from_block()` — 给定 block 和其渲染位置，计算并填充 TextMap

关键问题：block 渲染在哪个 x 坐标？Markdown 内容通常从 x=0 开始渲染（conversation area 左边界）。

先做最小化实现：只对 Markdown block 中的纯文本构建映射，不处理代码块、Welcome 等特殊 block。

**Step 6 简化实现**：

在 terminal.rs 中添加：

```rust
fn build_text_map(&mut self) {
    let text_map = self.renderer.text_map_mut();
    text_map.clear();
    
    // 计算 conversation area 的 x 偏移量
    // 从 layout.rs 或直接硬编码（假设 conversation 从 x=0 开始）
    let conv_x: u16 = 0;
    
    // 遍历所有 blocks，计算每个 block 的渲染位置
    let scroll = self.conversation_state.scroll_offset;
    let mut y: u16 = 0;
    
    for (block_idx, block) in self.blocks.iter().enumerate() {
        if block_idx >= self.conversation_state.item_heights.len() {
            break;
        }
        let height = self.conversation_state.item_heights[block_idx];
        
        // 如果该 block 不在可见区域内，跳过
        if y + height <= scroll {
            y += height;
            continue;
        }
        
        // 该 block 的渲染起点（相对于 scroll）
        let visible_start = y.saturating_sub(scroll);
        
        // 填充该 block 的 TextMap
        self.map_block_to_text_map(block, block_idx, visible_start, conv_x, text_map);
        
        y += height;
    }
}

fn map_block_to_text_map(&self, block: &ContentBlock, block_idx: usize, start_y: u16, start_x: u16, text_map: &mut TextMap) {
    let conv_width = self.renderer.area().width;
    
    match block {
        ContentBlock::Markdown { nodes } => {
            let mut current_y = start_y;
            let mut col = start_x;
            
            for node in nodes {
                if let MarkdownNode::Paragraph { spans } = node {
                    for (span_idx, span) in spans.iter().enumerate() {
                        let mut byte_offset = 0;
                        let mut line_width: u16 = 0;
                        
                        for ch in span.text.chars() {
                            let ch_width = char_width(ch) as u16;
                            
                            // 换行处理
                            if col + ch_width > conv_width {
                                current_y += 1;
                                col = start_x;
                                line_width = 0;
                            }
                            
                            // 跳过不可见行（scroll 导致的部分可见）
                            if current_y < self.renderer.area().height {
                                let source = CellSource {
                                    block: block_idx,
                                    span: span_idx,
                                    byte_offset,
                                    width: ch_width,
                                };
                                text_map.set_source(col, current_y, source);
                            }
                            
                            col += ch_width;
                            line_width += ch_width;
                            byte_offset += ch.len_utf8();
                        }
                    }
                }
            }
        }
        ContentBlock::Welcome { .. } | ContentBlock::ToolCall { .. } | ContentBlock::UserInput { .. } => {
            // 这些 block 也需要处理，但暂时跳过
        }
    }
}
```

这个实现不完美（不处理 word wrap 边界、特殊 block），但可以作为 Phase 2 的基础，让 Ctrl+C 能复制选中的文本。

Run: `cargo build` 并修复编译错误。

Run: `cargo test`

Commit

---

## Task 3: Ctrl+C 复制实际文本

**Files:**
- Modify: `src/tui/terminal.rs:592-600`

- [ ] **Step 1: 修改 Ctrl+C 处理**

将：
```rust
if let Some(region) = self.selection_state.region() {
    eprintln!("Selection: ({},{})-({},{})",
        region.top_left.0, region.top_left.1,
        region.bottom_right.0, region.bottom_right.1);
}
```

改为：
```rust
if let Some(text) = self.extract_selection_text() {
    eprintln!("Selected text:\n{}", text);
}
```

实现 `extract_selection_text()`:

```rust
fn extract_selection_text(&self) -> Option<String> {
    let region = self.selection_state.region()?;
    let text_map = self.renderer.text_map();
    
    // 收集所有被选中的 CellSource
    let mut sources: Vec<(&CellSource, (u16, u16))> = Vec::new();
    
    for row in region.top_left.1..=region.bottom_right.1 {
        for col in region.top_left.0..=region.bottom_right.0 {
            if let Some(source) = text_map.get_source(col, row) {
                sources.push((source, (col, row)));
            }
        }
    }
    
    if sources.is_empty() {
        return Some(String::new());
    }
    
    // 按 (block, span, byte_offset) 排序
    sources.sort_by_key(|(s, _)| (s.block, s.span, s.byte_offset));
    
    // 提取文本（需要根据 blocks 结构拼接）
    let mut result = String::new();
    let mut current_block = None;
    let mut current_span = None;
    let mut pending_chars: String = String::new();
    
    for (source, _) in &sources {
        // 简单的实现：直接从 block 的 span 文本中按字节偏移提取
        if let Some(block) = self.blocks.get(source.block) {
            // 这里需要根据 block 类型和 span 索引提取文本
            // 暂时用占位符
            if current_block != Some(source.block) {
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
                current_block = Some(source.block);
            }
        }
    }
    
    Some(result)
}
```

这个简化版本需要配合 Task 2 的 TextMap 构建才能真正提取文本。

**更实用的 Phase 2 MVP**：

先实现一个占位符版本，验证 Ctrl+C 行为：
```rust
if let Some(region) = self.selection_state.region() {
    eprintln!("Selection: ({},{})-({},{})", region.top_left.0, region.top_left.1,
        region.bottom_right.0, region.bottom_right.1);
    eprintln!("(Text extraction will be implemented in Phase 2 full)");
}
```

然后在 Phase 2 真正完成时替换为完整实现。

**重新考虑**：Phase 2 的目标就是提取文本。所以需要完整实现。

**完整 extract_selection_text 实现**：

```rust
fn extract_selection_text(&self) -> Option<String> {
    let region = self.selection_state.region()?;
    let text_map = self.renderer.text_map();
    
    #[derive(Debug, Clone)]
    struct SelectionEntry {
        block: usize,
        span: usize,
        byte_offset: usize,
        width: u16,
    }
    
    // 收集所有选中的 CellSource，按 (block, span, byte_offset) 排序
    let mut entries: Vec<SelectionEntry> = Vec::new();
    
    for row in region.top_left.1..=region.bottom_right.1 {
        for col in region.top_left.0..=region.bottom_right.0 {
            if let Some(source) = text_map.get_source(col, row) {
                entries.push(SelectionEntry {
                    block: source.block,
                    span: source.span,
                    byte_offset: source.byte_offset,
                    width: source.width,
                });
            }
        }
    }
    
    if entries.is_empty() {
        return Some(String::new());
    }
    
    // 去重并排序
    entries.sort_by_key(|e| (e.block, e.span, e.byte_offset));
    entries.dedup_by(|a, b| a.block == b.block && a.span == b.span && a.byte_offset == b.byte_offset);
    
    // 提取文本
    let mut result = String::new();
    let mut current_block = None;
    let mut current_span = None;
    let mut in_text = false;
    
    for entry in &entries {
        let block = self.blocks.get(entry.block)?;
        
        // 获取 span 文本
        let span_text = Self::get_span_text(block, entry.span)?;
        
        // 提取从 byte_offset 开始的字符
        let Some(text_bytes) = span_text.as_bytes().get(entry.byte_offset..) else {
            continue;
        };
        let Ok(ch) = std::str::from_utf8(text_bytes) else { continue; };
        let Some(first_char) = ch.chars().next() else { continue; };
        
        // 检测块/span 切换，添加换行
        if current_block != Some(entry.block) || current_span != Some(entry.span) {
            if in_text && !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            current_block = Some(entry.block);
            current_span = Some(entry.span);
        }
        
        result.push(first_char);
        in_text = true;
    }
    
    Some(result)
}

fn get_span_text(block: &ContentBlock, span_idx: usize) -> Option<String> {
    match block {
        ContentBlock::Markdown { nodes } => {
            let mut idx = 0;
            for node in nodes {
                if let MarkdownNode::Paragraph { spans } = node {
                    if idx + spans.len() > span_idx {
                        return Some(spans[span_idx - idx].text.clone());
                    }
                    idx += spans.len();
                }
            }
            None
        }
        _ => None,
    }
}
```

这个实现不完美，但作为 Phase 2 MVP 可以工作。

Run: `cargo build` 并修复编译错误。

Run: `cargo test`

Commit

---

## Task 4: 完整测试

- [ ] **Step 1: 完整测试**

```bash
cargo build
cargo test
cargo fmt
cargo clippy -- -D warnings
```

- [ ] **Step 2: 手动测试**

启动 `cargo run`，测试：
- 在 conversation 区域拖拽选择文本
- 按 Ctrl+C，验证输出的是选中的实际文本内容（而非坐标）
- 测试跨行、跨 block 选择

---

## 验收标准

- [ ] TextMap 构建正确（每帧清除并重建）
- [ ] Ctrl+C 打印选中的实际文本内容
- [ ] 跨行、跨 block 选择能正确拼接文本
- [ ] 所有 1070+ 测试通过
- [ ] cargo clippy 无警告
