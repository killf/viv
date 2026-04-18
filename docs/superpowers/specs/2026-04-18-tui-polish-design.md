# TUI 单窗口体验打磨 — 设计文档

## 概述

打磨现有 TUI 单窗口体验，修复 5 个已知问题：Markdown 换行、Unicode 宽度、ToolCall 内部滚动、代码块截断指示、交互式权限提示。

## 问题 1 & 2: Markdown 换行 + Unicode 宽度

### 现状

- `markdown.rs` 的 `render_inline_spans()` 逐字符写入 buffer，`cur_x >= max_x` 时 break（截断）
- `node_height()` 对 Paragraph/Heading/Quote/List item 硬编码返回 1
- `paragraph.rs` 已有完善的 `wrap_line()` 函数，支持 word-wrap + `char_width()` 宽字符
- `code_block.rs` 和 `tool_call.rs` 中手动渲染字符时用 `x += 1`，不处理宽字符

### 方案

MarkdownBlockWidget 的渲染路径从"逐字符写 buffer"改为"构建 Line → 用 Paragraph 渲染"，复用已有的 wrap 机制。

### 改动: paragraph.rs

导出换行辅助函数，避免暴露 `StyledChar` 内部类型：

```rust
/// 计算一个 Line 在指定宽度下 word-wrap 后的物理行数。
pub fn wrapped_line_count(line: &Line, width: usize) -> usize {
    wrap_line(line, width).len()
}
```

### 改动: markdown.rs

**删除** `render_inline_spans()` 函数。

**重写 `node_height()`**:

```rust
fn node_height(node: &MarkdownNode, width: u16) -> u16 {
    let w = width as usize;
    match node {
        MarkdownNode::Heading { text, .. } => {
            let line = spans_to_line(text, true);
            wrapped_line_count(&line, w) as u16
        }
        MarkdownNode::Paragraph { spans } => {
            let line = spans_to_line(spans, false);
            wrapped_line_count(&line, w) as u16
        }
        MarkdownNode::List { ordered, items } => {
            let mut total = 0u16;
            for (idx, item) in items.iter().enumerate() {
                let prefix = if *ordered {
                    format!("  {}. ", idx + 1)
                } else {
                    "  \u{2022} ".to_string() // "  • "
                };
                let prefix_len = display_width(&prefix) as usize;
                let line = spans_to_line(item, false);
                // 有效宽度 = 总宽度 - 前缀宽度
                let effective_w = w.saturating_sub(prefix_len);
                total += wrapped_line_count(&line, effective_w).max(1) as u16;
            }
            total
        }
        MarkdownNode::Quote { spans } => {
            let line = spans_to_line(spans, false);
            // "│ " 占 2 列
            wrapped_line_count(&line, w.saturating_sub(2)) as u16
        }
        MarkdownNode::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        MarkdownNode::HorizontalRule => 1,
    }
}
```

新增辅助函数 `spans_to_line()`，将 `&[InlineSpan]` 转为 `Line`（复用现有的 `inline_span_to_span()`）。

**重写 `MarkdownBlockWidget::render()`**:

每个 node 构建 `Vec<Line>` 后通过 `Paragraph` 渲染到子区域：

- **Heading**: 构建 bold Line → `Paragraph::new(vec![line]).render(sub_area, buf)`
- **Paragraph**: spans → Line → Paragraph 渲染
- **List**: 每个 item 构建带前缀 span 的 Line → Paragraph 渲染
- **Quote**: 前缀 `│ ` span + dim content spans → Line → Paragraph 渲染
- **CodeBlock / HorizontalRule**: 保持现有逻辑不变

row 步进改为 `row += node_height(node, area.width)`，与 height 计算一致。

### 改动: code_block.rs (Unicode 宽度)

token 渲染中 `x += 1` 改为 `x += char_width(ch)`：

```rust
for ch in token.text.chars() {
    let w = char_width(ch);
    if x + w > max_x {
        break;
    }
    let cell = buf.get_mut(x, y);
    cell.ch = ch;
    cell.fg = Some(fg);
    cell.bold = bold;
    if w == 2 && x + 1 < max_x {
        let cell2 = buf.get_mut(x + 1, y);
        cell2.ch = '\0';
        cell2.fg = Some(fg);
        cell2.bold = bold;
    }
    x += w;
}
```

### 改动: tool_call.rs (Unicode 宽度)

- `render_header()` 中 `name.len() as u16` → 用 display width 计算（遍历 chars 求 char_width 之和）
- `truncate_str()` 改为按显示宽度截断（累加 `char_width` 而非 char count）
- `status_str` 的长度计算也改用 display width

新增辅助函数：

```rust
fn display_width(s: &str) -> u16 {
    s.chars().map(|ch| char_width(ch)).sum()
}

fn truncate_str_by_width(s: &str, max_width: usize) -> &str {
    let mut width = 0;
    for (byte_idx, ch) in s.char_indices() {
        let w = char_width(ch) as usize;
        if width + w > max_width {
            return &s[..byte_idx];
        }
        width += w;
    }
    s
}
```

## 问题 3: ToolCall 展开后显示 output + 内部滚动

### 现状

展开 ToolCall 只显示 input block。`output_scroll` 字段预留但未接入。ToolCallWidget 不接收 output 数据。

### 改动: tool_call.rs

**ToolCallWidget 新增 output 字段**:

```rust
pub struct ToolCallWidget<'a> {
    name: &'a str,
    input_summary: &'a str,
    input_raw: &'a str,
    output: Option<&'a str>,  // 新增
    focused: bool,
}
```

**展开后的布局**:

```
┃ ⚙ ToolName summary              ✓ 1234 chars   ← header (1行)
 ╭─ input ─────────────────────────────────────╮
 │  {"file_path": "/src/foo.rs", ...}          │  ← input block
 ╰─────────────────────────────────────────────╯
 ╭─ output ────────────────────────────────────╮
 │  line 1                                     │  ← output block (可滚动)
 │  line 2                                     │
 ╰─────────────────────────────────────────────╯
```

**新增 `render_output()` 方法**:

- 渲染 output block，带 `" output "` 标题
- 内容区最多显示 10 行（`OUTPUT_MAX_VISIBLE = 10`）
- 用 `state.output_scroll` 控制偏移：`lines.iter().skip(scroll).take(max_visible)`
- 如果 output 行数 > max_visible，在边框上显示滚动指示（如 `▲`/`▼`）

**render_expanded() 改为渲染 input + output**:

```rust
fn render_expanded(&self, area: Rect, buf: &mut Buffer, state: &ToolCallState) {
    // 1. 渲染 input block（同现有逻辑）
    // 2. 如果 self.output.is_some()，在 input block 下方渲染 output block
}
```

**ToolCallState 新增滚动方法**:

```rust
impl ToolCallState {
    pub fn scroll_output_up(&mut self) { ... }
    pub fn scroll_output_down(&mut self, total_lines: usize) { ... }
    pub fn output_max_scroll(&self, total_lines: usize) -> u16 { ... }
}
```

**高度计算**:

```rust
pub fn expanded_height(input: &str, output: Option<&str>) -> u16 {
    let input_lines = input.split('\n').count() as u16;
    let input_block = input_lines + 2; // borders
    let output_block = match output {
        Some(o) => {
            let output_lines = o.split('\n').count() as u16;
            output_lines.min(OUTPUT_MAX_VISIBLE) + 2 // borders
        }
        None => 0,
    };
    1 + input_block + output_block // header + input + output
}
```

### 改动: bus/terminal.rs

- `render_block()` 中构造 ToolCallWidget 时传入 output
- `recalculate_tool_block_height()` 使用新的 `expanded_height()`
- `handle_key()` Browse 模式下，聚焦已展开 ToolCall 时 j/k 先滚动 output 内部，到边界后再滚动 conversation

Browse 模式 j/k 逻辑变更：

```
if 聚焦的 ToolCall 已展开 && output 超过 OUTPUT_MAX_VISIBLE 行:
    j → tool_states[idx].scroll_output_down(total_lines)
         如果已到底 → conversation_state.scroll_down(1)
    k → tool_states[idx].scroll_output_up()
         如果已到顶 → conversation_state.scroll_up(1)
else:
    j/k → conversation 全局滚动（保持现有行为）
```

## 问题 4: 代码块长行截断指示

### 现状

`CodeBlockWidget::render()` 中 `x >= max_x` 时 break，长行无任何视觉提示。

### 改动: code_block.rs

每行渲染完 token 后，检查是否有未渲染的内容。如果有，在 `max_x - 1` 位置写入 `→`：

```rust
// 在每行的 token 循环后
let mut line_overflowed = false;
'token_loop: for token in tokens {
    for ch in token.text.chars() {
        let w = char_width(ch) as u16;
        if x + w > max_x {
            line_overflowed = true;
            break 'token_loop;
        }
        // ... 写入 cell ...
        x += w;
    }
}
if line_overflowed {
    let indicator_x = max_x - 1;
    let cell = buf.get_mut(indicator_x, y);
    cell.ch = '\u{2192}'; // →
    cell.fg = Some(DIM_COLOR);
    cell.bold = false;
}
```

height() 不变，仍按 `line_count + 2` 计算。

## 问题 5: 交互式权限提示

### 现状

`PermissionRequest` 被构造为纯文本 Markdown 块。`permission.rs` 有渲染函数但未使用。

### 改动: content.rs

ContentBlock 新增变体：

```rust
pub enum ContentBlock {
    UserMessage { text: String },
    Markdown { nodes: Vec<MarkdownNode> },
    CodeBlock { language: Option<String>, code: String },
    ToolCall { id: usize, name: String, input: String, output: Option<String>, error: Option<String> },
    Permission { tool: String, summary: String, result: Option<bool> },  // 新增
}
```

### 改动: bus/terminal.rs

**handle_agent_message(PermissionRequest)**:

```rust
AgentMessage::PermissionRequest { tool, input } => {
    self.blocks.push(ContentBlock::Permission {
        tool: tool.clone(),
        summary: input.clone(),
        result: None,
    });
    let idx = self.blocks.len() - 1;
    self.conversation_state.append_item_height(1);
    self.pending_permission = Some((idx, tool, input));
    self.conversation_state.auto_scroll();
}
```

**handle_key 权限响应**:

```rust
// y/n 后修改 block 的 result 字段
if let ContentBlock::Permission { result, .. } = &mut self.blocks[idx] {
    *result = Some(allowed);
}
```

**render_block() 新增 Permission 分支**:

```rust
ContentBlock::Permission { tool, summary, result } => {
    let line = match result {
        None => render_permission_pending(tool, summary),
        Some(true) => render_permission_result(tool, summary, true),
        Some(false) => render_permission_result(tool, summary, false),
    };
    Paragraph::new(vec![line]).render(area, buf);
}
```

**block_height_with_width()**:

```rust
ContentBlock::Permission { .. } => 1,
```

### 改动: permission.rs

无需修改。现有的 `render_permission_pending()` 和 `render_permission_result()` 返回 `Line`，正好可以交给 Paragraph 渲染。

## 改动范围总结

| 文件 | 改动类型 | 大小 |
|------|----------|------|
| `paragraph.rs` | 新增 `wrapped_line_count()` pub 函数 | 小 |
| `markdown.rs` | 重写渲染路径 + node_height | 大 |
| `code_block.rs` | char_width 修复 + 截断指示符 | 小 |
| `tool_call.rs` | output 渲染 + 滚动 + Unicode 修复 | 大 |
| `content.rs` | 新增 Permission 变体 | 小 |
| `permission.rs` | 无改动 | 无 |
| `bus/terminal.rs` | 适配 Permission + ToolCall output + 高度 + 滚动 | 中 |

## 测试策略

- **markdown_test**: 新增换行场景测试（长段落、CJK 文本、混合 inline span）
- **tool_call_test**: 新增 expanded with output 测试、滚动边界测试
- **code_block_test**: 新增长行截断指示测试
- **content_test**: Permission 变体的序列化/构造测试
- **paragraph_test**: `wrapped_line_count()` 的边界测试（空行、宽字符、单词恰好填满）
