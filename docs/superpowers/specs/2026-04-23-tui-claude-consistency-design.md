# TUI 与 Claude Code 完全一致优化设计

**日期**: 2026-04-23
**状态**: 已批准
**目标**: Phase 3 - Diff 高亮、流式优化、消息折叠

---

## 1. 概述

### 1.1 目标
让 viv TUI 与 Claude Code 在视觉和行为上完全一致。Phase 3 聚焦于：
- Diff 高亮（文件变更着色）
- 流式渲染优化（逐行显示减少闪烁）
- 消息折叠/展开（PageUp/Down 滚动浏览）

### 1.2 参考实现
- **Claude Code**: TypeScript/Ink，基于 VirtualMessageList 的虚拟滚动
- **viv**: 纯 Rust，基于双缓冲 Buffer + LiveRegion

---

## 2. 架构设计

### 2.1 分层结构

```
┌─────────────────────────────────────────────────┐
│  TerminalSimulator (测试层)                      │
│  ├── send_message(AgentMessage)                │
│  ├── send_key(KeyEvent)                        │
│  └── screen() → ScreenState (断言接口)          │
├─────────────────────────────────────────────────┤
│  TerminalUI (渲染层)                             │
│  ├── LiveRegion                                 │
│  │   ├── scrollback: Vec<CommittedBlock>      │
│  │   ├── live: Vec<LiveBlock>                 │
│  │   └── fold_state: HashMap<Id, FoldState>    │
│  ├── Editor                                     │
│  └── StatusBar                                  │
├─────────────────────────────────────────────────┤
│  Backend (终端层)                               │
│  ├── Buffer (双缓冲 diff 渲染)                 │
│  └── ANSI (颜色序列生成)                        │
└─────────────────────────────────────────────────┘
```

### 2.2 数据流

```
AgentMessage ──→ TerminalUI ──→ LiveRegion ──→ Renderer ──→ Backend ──→ Terminal
                     ↑
KeyEvent ───────────┘
```

---

## 3. 功能规格

### 3.1 Diff 高亮

**目的**: Read 文件、Edit 变更时显示红绿差异

**数据格式**:
```rust
enum AgentMessage {
    // 新增
    DiffView {
        hunks: Vec<DiffHunk>,
    },
}

// DiffHunk 结构
struct DiffHunk {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    lines: Vec<DiffLine>,
}

enum DiffLine {
    Context(String),   // 白色
    Addition(String), // 绿色 (#ansi_green)
    Deletion(String),  // 红色 (#ansi_red)
}
```

**ANSI 序列**:
- 删除: `\x1b[31m` (红色前景)
- 添加: `\x1b[32m` (绿色前景)
- 行号: `\x1b[90m` (亮黑色)
- 重置: `\x1b[0m`

**渲染位置**: LiveRegion 内的 CommittedBlock

**测试断言**:
```rust
assert!(screen.has_ansi_sequence("\x1b[31m")); // 红色
assert!(screen.has_ansi_sequence("\x1b[32m")); // 绿色
```

### 3.2 流式渲染优化

**目的**: 逐行追加文本，减少视觉闪烁

**当前问题**:
- 每次 TextChunk 触发完整重绘
- 光标位置抖动

**优化方案**:
```rust
// content.rs - TextChunk 处理
pub fn push(&mut self, text: &str) -> Vec<ContentBlock> {
    let mut new_blocks = Vec::new();
    let mut line_buffer = String::new();

    for chunk in text.chars() {
        if chunk == '\n' {
            // 只在遇到换行时才提交整行
            if !line_buffer.is_empty() {
                new_blocks.push(ContentBlock::Markdown {
                    nodes: parse_line(&line_buffer),
                });
                line_buffer.clear();
            }
        } else {
            line_buffer.push(chunk);
        }
    }

    // 未完成的行留在 pending
    self.pending = line_buffer;
    new_blocks
}
```

**渲染行为**:
1. 遇到 `\n` → 提交完整行到 LiveRegion
2. 未完成的行 → 单独 LiveBlock，state=Live
3. 下一 TextChunk → 追加到 LiveBlock

**测试断言**:
```rust
sim.send_message(AgentMessage::TextChunk("Hello\n".into()));
assert!(sim.screen().line_contains(10, "Hello"));

sim.send_message(AgentMessage::TextChunk("World\n".into()));
// Hello 已提交为 committed，World 在 live
assert!(sim.screen().line_contains(11, "World"));
```

### 3.3 消息折叠/展开

**目的**: PageUp/Down 浏览历史，ToolCall 可折叠

**数据结构**:
```rust
struct FoldState {
    collapsed: bool,
    visible_lines: u16,
}

enum FoldableBlock {
    ToolCall {
        id: usize,
        state: FoldState,
        // ...
    },
    DiffView {
        state: FoldState,
        // ...
    },
}
```

**交互**:
| 按键 | 行为 |
|------|------|
| PageUp | 向上滚动视图，显示历史消息 |
| PageDown | 向下滚动视图 |
| `+` / `-` | 折叠/展开当前 ToolCall |
| `a` (在折叠项上) | 展开所有 |

**渲染偏移**:
```rust
struct RenderContext {
    scroll_offset: u16,      // 滚动偏移量
    viewport_height: u16,    // 可见区域高度
}

fn frame(&mut self, ctx: &RenderContext) -> CursorPos {
    // 只渲染 scroll_offset 到 scroll_offset + viewport_height 的内容
}
```

**测试断言**:
```rust
// 产生 30 条消息
for i in 0..30 {
    sim.send_message(AgentMessage::TextChunk(format!("Line {}\n", i)));
}

sim.send_key(KeyEvent::PageUp);
let screen = sim.screen();
// 应该看到历史消息，不是最新消息
assert!(screen.line_text(20).contains("Line 20"));
```

---

## 4. 文件变更

| 文件 | 变更类型 | 描述 |
|------|---------|------|
| `src/agent/protocol.rs` | 新增 | `DiffView` 消息类型 |
| `tests/tui/simulator_test.rs` | 扩展 | Phase 3 测试用例 |
| `tests/tui/live_region_test.rs` | 扩展 | 折叠状态测试 |
| `src/tui/live_region.rs` | 修改 | 添加折叠状态管理 |
| `src/tui/content.rs` | 修改 | 流式逐行解析 |
| `src/tui/ansi_serialize.rs` | 修改 | Diff 颜色序列 |
| `src/tui/renderer.rs` | 修改 | 滚动偏移支持 |
| `src/tui/input.rs` | 修改 | PageUp/Down 按键 |

---

## 5. 测试策略

### 5.1 测试先行
1. 在 `tests/tui/` 写失败测试
2. 运行 `cargo test` 确认失败
3. 实现功能使测试通过
4. 迭代直到全部通过

### 5.2 TerminalSimulator API 扩展
```rust
impl TerminalSimulator {
    // 现有 API
    pub fn send_message(&mut self, msg: AgentMessage);
    pub fn send_key(&mut self, key: KeyEvent);
    pub fn screen(&self) -> &Screen;
    pub fn input_content(&self) -> &str;
    pub fn input_mode(&self) -> InputMode;

    // 新增 API
    pub fn scroll_offset(&self) -> u16;
    pub fn assert_ansi_color(&self, row: u16, color: AnsiColor);
    pub fn folded_blocks(&self) -> Vec<FoldableBlock>;
}
```

### 5.3 测试用例

```rust
// diff_test.rs
mod diff_highlighting {
    #[test]
    fn diff_view_renders_red_deletions() { /* ... */ }
    #[test]
    fn diff_view_renders_green_additions() { /* ... */ }
    #[test]
    fn diff_view_shows_line_numbers() { /* ... */ }
}

// streaming_test.rs
mod line_by_line_streaming {
    #[test]
    fn text_chunk_submits_on_newline() { /* ... */ }
    #[test]
    fn pending_line_is_live() { /* ... */ }
    #[test]
    fn completed_lines_commit() { /* ... */ }
}

// fold_test.rs
mod message_folding {
    #[test]
    fn pageup_scrolls_history() { /* ... */ }
    #[test]
    fn pagedown_scrolls_forward() { /* ... */ }
    #[test]
    fn tool_call_collapsible() { /* ... */ }
    #[test]
    fn expand_all_shows_content() { /* ... */ }
}
```

---

## 6. 实现顺序

```
Week 1: Diff 高亮
├── 定义 DiffView 消息类型
├── 添加 ANSI 颜色序列支持
├── 实现 DiffBlock 渲染
└── 编写 diff_test.rs

Week 2: 流式优化
├── 修改 content.rs 逐行解析
├── 调整 LiveRegion 处理 Live vs Committed
├── 优化渲染减少闪烁
└── 编写 streaming_test.rs

Week 3: 消息折叠
├── 实现 PageUp/Down 滚动
├── 添加 ToolCall 折叠状态
├── 实现折叠/展开交互
└── 编写 fold_test.rs
```

---

## 7. 验收标准

- [ ] `cargo test --test simulator_test` 全部通过
- [ ] `cargo test` 全部通过（无警告）
- [ ] Diff 视图显示红绿差异
- [ ] 流式输出无明显闪烁
- [ ] PageUp/Down 正确滚动
- [ ] ToolCall 可折叠/展开
