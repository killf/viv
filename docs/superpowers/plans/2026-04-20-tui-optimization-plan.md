# TUI 优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 优化 viv TUI 的视觉效果和交互体验：欢迎动画、呼吸动画、输入历史、全局滚动、鼠标支持

**Architecture:** 在现有 Widget/Buffer 渲染引擎基础上增量修改，不改动核心架构。输入历史存于 LineEditor，滚动由 ConversationState 管理，动画状态存于 TerminalUI。

**Tech Stack:** 纯 Rust，无外部依赖

---

## 阶段一：输入历史

### Task 1: LineEditor 增加 history 字段

**Files:**
- Modify: `src/tui/terminal.rs`（LineEditor 部分）

- [ ] **Step 1: 在 LineEditor 结构体中增加三个字段**

在 `LineEditor` 定义后补充：

```rust
pub struct LineEditor {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    // 新增字段
    history: Vec<String>,
    history_idx: Option<usize>,
    original: String,
}
```

- [ ] **Step 2: 初始化新字段**

在 `LineEditor::new()` 中：

```rust
pub fn new() -> Self {
    LineEditor {
        lines: vec![String::new()],
        row: 0,
        col: 0,
        history: Vec::new(),
        history_idx: None,
        original: String::new(),
    }
}
```

- [ ] **Step 3: 实现 history_push 方法**

在 `LineEditor` impl 块末尾添加：

```rust
/// Push a submitted line into history.
pub fn push_history(&mut self, line: String) {
    if !line.is_empty() {
        self.history.push(line);
    }
}
```

- [ ] **Step 4: 修改 handle_key 处理 ↑↓**

找到 `handle_key` 中 `KeyEvent::Up` 和 `KeyEvent::Down` 的现有处理逻辑，替换为：

```rust
KeyEvent::Up => {
    // History browsing
    if self.history.is_empty() {
        return EditAction::Continue;
    }
    if self.history_idx.is_none() {
        // Save current input
        self.original = self.content();
        self.history_idx = Some(self.history.len().saturating_sub(1));
        let last = self.history[self.history_idx.unwrap()].clone();
        self.lines = vec![last];
        self.row = 0;
        self.col = self.lines[0].len();
    } else if let Some(idx) = self.history_idx {
        if idx > 0 {
            self.history_idx = Some(idx - 1);
            self.lines = vec![self.history[idx - 1].clone()];
            self.row = 0;
            self.col = self.lines[0].len();
        }
    }
    EditAction::Continue
}
KeyEvent::Down => {
    if let Some(idx) = self.history_idx {
        if idx + 1 < self.history.len() {
            self.history_idx = Some(idx + 1);
            self.lines = vec![self.history[idx + 1].clone()];
            self.row = 0;
            self.col = self.lines[0].len();
        } else {
            // Back to current input
            self.history_idx = None;
            self.lines = vec![self.original.clone()];
            self.row = 0;
            self.col = self.lines[0].len();
        }
    }
    EditAction::Continue
}
```

- [ ] **Step 5: 编辑操作退出历史浏览**

在 `handle_key` 的 `Char`、`Backspace`、`Delete`、`Home`、`End` 等编辑操作分支开头添加：

```rust
KeyEvent::Char(ch) => {
    // Exit history browsing on any edit
    if self.history_idx.is_some() {
        // Restore original content as base for editing
        self.lines = vec![self.original.clone()];
        self.row = 0;
        self.col = self.lines[0].len();
        self.history_idx = None;
    }
    // ... 原有逻辑
}
```

同样处理 `Backspace`、`Delete`、`Home`、`End`、`Left`、`Right`、`ShiftEnter`。

- [ ] **Step 6: 提交时 push_history 并清空浏览状态**

在 `EditAction::Submit` 分支中：

```rust
EditAction::Submit(line) => {
    if !line.trim().is_empty() {
        self.push_history(line.clone()); // 新增
        // ... existing
    }
    // 清空 history 浏览状态
    self.history_idx = None;
    self.original.clear();
    // ...
}
```

- [ ] **Step 7: 修改 TerminalUI 中 Submit 处理，push_history**

在 `TerminalUI::handle_key` 的 `EditAction::Submit` 分支中，`self.event_tx.send` 前添加：

```rust
self.editor.push_history(line.clone());
```

- [ ] **Step 8: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 9: 提交**

```bash
git add src/tui/terminal.rs
git commit -m "feat(tui): add input history with up/down arrow browsing"
```

---

## 阶段二：鼠标支持

### Task 2: InputParser 解析 SGR 鼠标序列

**Files:**
- Modify: `src/core/terminal/input.rs`

- [ ] **Step 1: 在 KeyEvent 之后添加 MouseEvent 枚举**

```rust
/// Mouse event type.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseEvent {
    WheelUp,
    WheelDown,
    LeftPress,
    LeftRelease,
}
```

- [ ] **Step 2: 在 InputParser 中增加小鼠缓冲字段**

在 `InputParser` 结构体中添加：

```rust
pub struct InputParser {
    pub buf: Vec<u8>,
    mouse_buf: Vec<u8>, // 收集 SGR 鼠标参数
}
```

- [ ] **Step 3: 修改 `next_event` 处理 SGR 鼠标序列**

在 `0x1b`（Escape）分支中，找到 `b'['` 之后，在所有 `Some(_)` 的 CSI 处理之后（未匹配的情况）添加鼠标解析：

```rust
Some(_) => {
    // 尝试解析 SGR 鼠标序列: ESC [ < N ; X ; Y M 或 m
    // 先检查是否是 SGR 鼠标 (< 前缀)
    if self.buf.get(2) == Some(&b'<') {
        // 收集完整的 SGR 序列
        let mut end = 3;
        while end < self.buf.len() && end < 50 {
            let ch = self.buf[end];
            if ch == b'M' || ch == b'm' {
            end += 1;
            break;
            }
            end += 1;
        }
        if self.buf.get(end - 1) == Some(&b'M') || self.buf.get(end - 1) == Some(&b'm') {
            let seq = &self.buf[..end];
            // 解析: ESC [ < N ; X ; Y M/m
            // 找 '<' 的位置
            if let Some(lt_pos) = seq.iter().position(|&b| b == b'<') {
                let params = &seq[lt_pos + 1..end - 1]; // 不含 < 和最后的 M/m
                let parts: Vec<&[u8]> = params.split(|&b| b == b';').collect();
                if parts.len() >= 3 {
                    let btn = parse_u8(parts[0]).unwrap_or(0);
                    if btn == 64 {
                        self.buf.drain(..end);
                        return Some(Event::Mouse(MouseEvent::WheelUp));
                    } else if btn == 65 {
                        self.buf.drain(..end);
                        return Some(Event::Mouse(MouseEvent::WheelDown));
                    } else if btn == 0 {
                        let last = seq[end - 1];
                        if last == b'M' {
                            self.buf.drain(..end);
                            return Some(Event::Mouse(MouseEvent::LeftPress));
                        } else {
                            self.buf.drain(..end);
                            return Some(Event::Mouse(MouseEvent::LeftRelease));
                        }
                    }
                }
            }
        }
        // 不是有效鼠标序列，当作未知 CSI 消费
        self.buf.drain(..3);
        return Some(Event::Unknown(self.buf.drain(..).collect()));
    }
    // 其他 CSI 序列
    let consumed: Vec<u8> = self.buf.drain(..3).collect();
    return Some(Event::Unknown(consumed));
}
```

注意：需要添加辅助函数：

```rust
fn parse_u8(s: &[u8]) -> Option<u8> {
    let mut val: u8 = 0;
    for &b in s {
        if b < b'0' || b > b'9' {
            return None;
        }
        val = val.checked_mul(10)?. + (b - b'0');
    }
    Some(val)
}
```

- [ ] **Step 4: 修改 InputParser::new 初始化 mouse_buf**

```rust
pub fn new() -> Self {
    InputParser {
        buf: Vec::new(),
        mouse_buf: Vec::new(),
    }
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 6: 提交**

```bash
git add src/core/terminal/input.rs
git commit -m "feat(input): parse SGR mouse sequences (wheel + click)"
```

---

### Task 3: Event 新增 Mouse 变体

**Files:**
- Modify: `src/core/event.rs`

- [ ] **Step 1: 添加 Mouse 导入和变体**

```rust
use super::input::{InputParser, KeyEvent, MouseEvent};

pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(TermSize),
    Tick,
}
```

- [ ] **Step 2: 修改 EventLoop::drain_stdin 处理 Mouse 事件**

在 `drain_stdin` 的 `while let Some(key) = self.input.next_event()` 循环中：

```rust
while let Some(event) = self.input.next_event() {
    match event {
        super::input::InputEvent::Key(k) => events.push(Event::Key(k)),
        super::input::InputEvent::Mouse(m) => events.push(Event::Mouse(m)),
    }
}
```

但 `next_event` 返回的是 `Option<KeyEvent>`，不是新的 `InputEvent` 枚举。最简方式：直接返回 `Option<Event>`。更好的方案是让 `next_event` 返回 `Option<InputEvent>`，其中 `InputEvent` 包含 `Key` 和 `Mouse`。

选择：修改 `next_event` 返回 `Option<Event>` 更简洁，因为 `Event` 在 `input.rs` 不可见。

改法：在 `input.rs` 中，`next_event` 返回 `Option<Event>`，但 `Event` 在 `input.rs` 不存在。最干净的做法：创建 `InputEvent` 枚举。

实际上最简单：**让 `InputParser.next_event()` 返回 `Option<InputEvent>`**，其中 `InputEvent` 定义在 `input.rs` 中，然后在 `event.rs` 中 match 并转为 `Event`。

```rust
// input.rs 新增:
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

pub fn next_event(&mut self) -> Option<InputEvent> { ... }
```

- [ ] **Step 3: 修改 input.rs next_event 返回 InputEvent**

将 `next_event` 的返回值从 `Option<KeyEvent>` 改为 `Option<InputEvent>`。所有 `Some(KeyEvent::...)` 改为 `Some(InputEvent::Key(...))`，鼠标解析返回 `Some(InputEvent::Mouse(...))`。

- [ ] **Step 4: 修改 event.rs 使用 InputEvent**

```rust
use super::input::{InputParser, InputEvent, MouseEvent, KeyEvent};

// drain_stdin 中:
while let Some(event) = self.input.next_event() {
    match event {
        InputEvent::Key(k) => events.push(Event::Key(k)),
        InputEvent::Mouse(m) => events.push(Event::Mouse(m)),
    }
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 6: 提交**

```bash
git add src/core/event.rs src/core/terminal/input.rs
git commit -m "feat(event): add Event::Mouse variant and InputEvent enum"
```

---

### Task 4: TerminalUI 处理鼠标滚轮事件

**Files:**
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: 在 Event::Key 分支后添加 Event::Mouse 处理**

在 `run()` 的事件循环中，找到 `Event::Resize` 分支之后添加：

```rust
Event::Mouse(MouseEvent::WheelUp) => {
    self.conversation_state.scroll_up(3);
}
Event::Mouse(MouseEvent::WheelDown) => {
    self.conversation_state.scroll_down(3);
}
Event::Mouse(_) => {
    // 其他鼠标事件暂不处理
}
```

需要导入 `MouseEvent`：
```rust
use crate::core::terminal::input::MouseEvent;
```

- [ ] **Step 2: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 3: 提交**

```bash
git add src/tui/terminal.rs
git commit -m "feat(tui): handle mouse wheel scroll globally"
```

---

## 阶段三：Ctrl+K/J 滚动

### Task 5: Ctrl+K/J 全局滚动

**Files:**
- Modify: `src/core/terminal/input.rs`
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: InputParser 支持 Ctrl+字母**

当前 `input.rs` 只处理了 Ctrl+C(3) 和 Ctrl+D(4)。需要扩展处理 Ctrl+A-Z (1, 2, 5-26)。

在 ASCII 0-31 处理分支末尾（`127` Backspace 之前）添加：

```rust
// Ctrl+A through Ctrl+Z: 0x01..=0x1A
1..=26 => {
    let ch = (first + b'a' - 1) as char; // Ctrl+A -> 'a', etc.
    self.buf.drain(..1);
    Some(KeyEvent::CtrlChar(ch))
}
```

添加新变体：
```rust
CtrlChar(char), // Ctrl+a through Ctrl+z
```

- [ ] **Step 2: TerminalUI 处理 Ctrl+K/J**

在 `handle_key` 函数顶部（Permission 和其他处理之前）添加：

```rust
KeyEvent::CtrlChar('k') => {
    self.conversation_state.scroll_up(3);
    return None;
}
KeyEvent::CtrlChar('j') => {
    self.conversation_state.scroll_down(3);
    return None;
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 4: 提交**

```bash
git add src/core/terminal/input.rs src/tui/terminal.rs
git commit -m "feat(tui): add Ctrl+K/J for global scroll navigation"
```

---

## 阶段四：欢迎页淡入动画

### Task 6: 欢迎页逐行淡入动画

**Files:**
- Modify: `src/tui/terminal.rs`
- Modify: `src/tui/welcome.rs`

- [ ] **Step 1: WelcomeWidget 支持带 alpha 渲染**

在 `WelcomeWidget` 中增加 `alpha` 参数（0.0=完全透明/dim，1.0=完全亮）。修改 `Widget` impl 中的 `render` 方法：

```rust
pub fn alpha_for_row(&self, row_idx: usize, elapsed_ms: u64, start_ms: u64) -> f32 {
    let delay_ms = (row_idx as u64).saturating_mul(80);
    if elapsed_ms < start_ms + delay_ms {
        return 0.0;
    }
    let progress = (elapsed_ms - start_ms - delay_ms) as f32 / 200.0;
    progress.min(1.0)
}
```

但 Widget trait 的 `render` 方法签名固定为 `&self`，不能访问时间。动画状态应存在 `TerminalUI` 中，只在 `render_frame` 时传入计算好的颜色。

方案：在 `TerminalUI.render_frame` 中，对 Welcome block 单独处理，根据动画时间计算每行的颜色。

- [ ] **Step 2: TerminalUI 增加 welcome_anim 状态**

在 `TerminalUI` 结构体中添加：

```rust
welcome_anim: Option<WelcomeAnimState>,

struct WelcomeAnimState {
    start: Instant,
    total_rows: u16,
}
```

在 `new()` 中初始化为 `Some(WelcomeAnimState { start: Instant::now(), total_rows: 5 })`。

在 `render_frame` 中判断：动画是否完成（超过 `5 * 80 + 200 = 600ms`），若完成则设 `welcome_anim = None`。

- [ ] **Step 3: 修改 render_block 中 Welcome 的渲染逻辑**

在 `render_block` 函数处理 `ContentBlock::Welcome` 的分支中，如果 `TerminalUI` 有 `welcome_anim` 状态，根据流逝时间计算每行亮度，传给 `WelcomeWidget::render_with_alpha`。

最简实现：直接改 `WelcomeWidget` 的 `render` 方法，用 `theme::CLAUDE_SHIMMER` 或多步灰度近似淡入：

```rust
// Logo 行（0-4）始终亮色
// Info 行（0-4）的 fg 根据动画进度调整:
// 0: DIM → TEXT 线性插值
let fg = if row >= 5 {
    let alpha = /* 计算 0..1 */;
    let dim = theme::DIM;
    let text = theme::TEXT;
    // 简单：alpha=0 用 DIM, alpha=1 用 TEXT
    if alpha < 0.5 { dim } else { text }
} else { theme::CLAUDE };
```

- [ ] **Step 4: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 5: 提交**

```bash
git add src/tui/terminal.rs src/tui/welcome.rs
git commit -m "feat(tui): add welcome screen fade-in animation"
```

---

## 阶段五：工具调用呼吸动画

### Task 7: 工具调用 Running 时呼吸动画

**Files:**
- Modify: `src/tui/terminal.rs`
- Modify: `src/tui/tool_call.rs`

- [ ] **Step 1: ToolCallState 增加 running_start 时间戳**

```rust
pub struct ToolCallState {
    pub folded: bool,
    pub status: ToolStatus,
    pub output_scroll: u16,
    pub running_start: Option<std::time::Instant>, // 新增
}
```

修改 `new_running()`：
```rust
pub fn new_running() -> Self {
    ToolCallState {
        folded: true,
        status: ToolStatus::Running,
        output_scroll: 0,
        running_start: Some(std::time::Instant::now()),
    }
}
```

- [ ] **Step 2: TerminalUI 中计算呼吸并传给 ToolCallWidget**

在 `render_frame` 中，找到处理 `ToolCall` 的 `render_block` 调用。为 `ToolCallWidget` 新增一个 `breathing` 参数或方法，根据 `running_start` 计算背景色：

```rust
// 计算呼吸 alpha: 0..1 正弦
fn breath_alpha(start: std::time::Instant) -> f32 {
    let elapsed = start.elapsed().as_millis() as f32;
    let phase = (elapsed % 1000.0) / 1000.0 * 2.0 * std::f32::consts::PI;
    (phase.sin() * 0.5 + 0.5)
}
```

在渲染 ToolCall block 时，如果是 Running 状态，根据 `breath_alpha` 混合背景色。可以在 `ToolCallWidget.render_header` 中检测 Running 状态并设置行背景色。

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 4: 提交**

```bash
git add src/tui/terminal.rs src/tui/tool_call.rs
git commit -m "feat(tui): add tool call breathing animation for running state"
```

---

## 阶段六：移除 Browse 模式

### Task 8: 简化 FocusManager，移除 UIMode

**Files:**
- Modify: `src/tui/terminal.rs`
- Modify: `src/tui/focus.rs`

- [ ] **Step 1: 移除 UIMode 枚举和 Browse 模式逻辑**

在 `focus.rs` 中删除 `UIMode` 枚举。删除 `enter_browse`、`exit_browse` 方法。`FocusManager` 简化为只管理工具焦点索引。

- [ ] **Step 2: TerminalUI 移除 Browse 模式相关代码**

删除：
- `Escape` 进入 Browse 模式的处理
- `handle_key` 中 Browse 模式的整个分支
- `quitting` 相关逻辑保持不变
- `focus: FocusManager` 字段保留（用于工具焦点高亮，但不再切换模式）

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1`
Expected: 无编译错误

- [ ] **Step 4: 提交**

```bash
git add src/tui/terminal.rs src/tui/focus.rs
git commit -m "refactor(tui): remove Browse/Normal mode, simplify focus management"
```

---

## 验证清单

- [ ] 输入历史：↑↓ 浏览，编辑退出历史，提交推送历史
- [ ] 鼠标滚轮：任意位置滚轮均可滚动对话
- [ ] Ctrl+K/J：全局滚动，auto_follow 自动禁用/恢复
- [ ] 欢迎动画：启动时逐行淡入，~600ms 后静态
- [ ] 呼吸动画：Running 工具调用行边框/背景呼吸
- [ ] 滚动条：右侧滚动条正确显示位置
- [ ] 终端 resize：窗口大小变化时重新计算布局

Run: `cargo test --lib 2>&1 | tail -20`
