# TerminalSimulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 TerminalSimulator，用于测试 viv TUI 的终端模拟器

**Architecture:** 模拟器通过内部维护与 TerminalUI 相同的状态，使用 TestBackend 捕获渲染输出，解析 ANSI 序列重建终端画面

**Tech Stack:** 纯 Rust，无外部依赖

---

## 文件结构

- Create: `src/core/terminal/simulator.rs` — 核心模拟器实现
- Modify: `src/core/terminal/mod.rs` — 导出新模块
- Modify: `tests/tui/mod.rs` — 添加测试模块

---

## Task 1: 创建基础数据结构

**Files:**
- Create: `src/core/terminal/simulator.rs`

- [ ] **Step 1: 创建文件，写入基础结构**

```rust
use super::backend::TestBackend;
use super::size::TermSize;
use crate::agent::protocol::AgentMessage;
use crate::tui::live_region::LiveRegion;
use crate::tui::status::StatusContext;
use crate::core::terminal::input::KeyEvent;

/// 颜色定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Ansi(u8),
    Rgb(u8, u8, u8),
}

/// 单元格样式
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}

/// 单个单元格
#[derive(Debug, Clone)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

/// 终端画面
#[derive(Debug, Clone)]
pub struct Screen<'a> {
    pub grid: &'a Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub cursor: (usize, usize),
}

impl<'a> Screen<'a> {
    /// 获取指定位置的单元格
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.grid.get(row)?.get(col)
    }

    /// 获取指定行
    pub fn line(&self, row: usize) -> Option<&[Cell]> {
        self.grid.get(row).map(|r| r.as_slice())
    }

    /// 检查画面是否包含文本（忽略样式）
    pub fn contains(&self, text: &str) -> bool {
        let plain: String = self.grid.iter()
            .flat_map(|row| row.iter().map(|c| c.ch))
            .collect();
        plain.contains(text)
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add src/core/terminal/simulator.rs
git commit -m "feat(tui): add TerminalSimulator basic data structures"
```

---

## Task 2: 实现 ANSI 解析器

**Files:**
- Modify: `src/core/terminal/simulator.rs`

- [ ] **Step 1: 添加解析器结构体和解析状态机**

在 `simulator.rs` 中添加：

```rust
/// ANSI 解析器状态机
struct AnsiParser {
    width: usize,
    height: usize,
    grid: Vec<Vec<Cell>>,
    cursor: (usize, usize),
    current_style: CellStyle,
    // 解析状态
    in_escape: bool,
    escape_buf: Vec<u8>,
}

impl AnsiParser {
    fn new(width: usize, height: usize) -> Self {
        let grid = vec![vec![Cell { ch: ' ', style: CellStyle::default() }; width]; height];
        AnsiParser {
            width,
            height,
            grid,
            cursor: (0, 0),
            current_style: CellStyle::default(),
            in_escape: false,
            escape_buf: Vec::new(),
        }
    }

    /// 追加字节并解析
    fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.parse_byte(b);
        }
    }

    fn parse_byte(&mut self, b: u8) {
        match b {
            0x1b => {
                self.in_escape = true;
                self.escape_buf.clear();
                self.escape_buf.push(b);
            }
            b if self.in_escape => {
                self.escape_buf.push(b);
                if self.is_escape_complete() {
                    self.handle_escape();
                    self.in_escape = false;
                    self.escape_buf.clear();
                }
            }
            b'\r' => {
                self.cursor.1 = 0;
            }
            b'\n' => {
                self.cursor.0 += 1;
                if self.cursor.0 >= self.height {
                    self.scroll_up();
                }
            }
            b'\t' => {
                let next = (self.cursor.1 / 8 + 1) * 8;
                self.cursor.1 = next.min(self.width - 1);
            }
            b if b.is_ascii_graphic() || b == b' ' => {
                self.put_cell(b as char);
            }
            _ => {}
        }
    }

    fn is_escape_complete(&self) -> bool {
        match self.escape_buf.first() {
            Some(&0x1b) => {
                // \x1b[... 完整
                matches!(self.escape_buf.last(), Some(&b'm') | Some(&b'H') | Some(&b'J') | Some(&b'K') | Some(&b'A') | Some(&b'B') | Some(&b'C') | Some(&b'D'))
            }
            _ => false,
        }
    }

    fn handle_escape(&mut self) {
        let buf = &self.escape_buf;
        if buf.len() < 3 || buf[0] != 0x1b || buf[1] != b'[' {
            return;
        }

        let params = &buf[2..buf.len()-1];
        let cmd = *buf.last().unwrap();

        match cmd {
            b'm' => self.handle_sgr(params),
            b'H' | b'f' => self.handle_cursor_pos(params),
            b'J' => self.handle_erase_display(params),
            b'K' => self.handle_erase_line(params),
            b'A' => self.handle_cursor_up(params),
            b'B' => self.handle_cursor_down(params),
            b'C' => self.handle_cursor_forward(params),
            b'D' => self.handle_cursor_back(params),
            _ => {}
        }
    }

    fn handle_sgr(&mut self, params: &[u8]) {
        if params.is_empty() || params == b"0" {
            self.current_style = CellStyle::default();
            return;
        }

        let params_str = String::from_utf8_lossy(params);
        let parts: Vec<&str> = params_str.split(';').collect();

        let mut i = 0;
        while i < parts.len() {
            match parts[i].parse::<u8>() {
                Ok(0) => self.current_style = CellStyle::default(),
                Ok(1) => self.current_style.bold = true,
                Ok(n) if (30..=37).contains(&n) => self.current_style.fg = Some(Color::Ansi(n)),
                Ok(n) if (90..=97).contains(&n) => self.current_style.fg = Some(Color::Ansi(n)),
                Ok(38) if i + 2 < parts.len() => {
                    // \x1b[38;2;r;g;bm
                    if parts[i+1] == "2" && i + 4 < parts.len() {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[i+2].parse::<u8>(),
                            parts[i+3].parse::<u8>(),
                            parts[i+4].parse::<u8>(),
                        ) {
                            self.current_style.fg = Some(Color::Rgb(r, g, b));
                            i += 4;
                        }
                    }
                }
                Ok(48) if i + 2 < parts.len() => {
                    // \x1b[48;2;r;g;bm
                    if parts[i+1] == "2" && i + 4 < parts.len() {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[i+2].parse::<u8>(),
                            parts[i+3].parse::<u8>(),
                            parts[i+4].parse::<u8>(),
                        ) {
                            self.current_style.bg = Some(Color::Rgb(r, g, b));
                            i += 4;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn handle_cursor_pos(&mut self, params: &[u8]) {
        let params_str = String::from_utf8_lossy(params);
        let parts: Vec<&str> = params_str.split(';').collect();

        let row = parts.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
        let col = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);

        self.cursor.0 = row.saturating_sub(1).min(self.height - 1);
        self.cursor.1 = col.saturating_sub(1).min(self.width - 1);
    }

    fn handle_cursor_up(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.0 = self.cursor.0.saturating_sub(n);
    }

    fn handle_cursor_down(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.0 = (self.cursor.0 + n).min(self.height - 1);
    }

    fn handle_cursor_forward(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.1 = (self.cursor.1 + n).min(self.width - 1);
    }

    fn handle_cursor_back(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.1 = self.cursor.1.saturating_sub(n);
    }

    fn handle_erase_display(&mut self, params: &[u8]) {
        let mode = Self::parse_param(params, 0);
        match mode {
            0 | 2 => {
                // 清从光标到屏幕结尾 或 清整屏
                for row in self.cursor.0..self.height {
                    self.clear_row(row);
                }
            }
            _ => {}
        }
    }

    fn handle_erase_line(&mut self, params: &[u8]) {
        let mode = Self::parse_param(params, 0);
        match mode {
            0 | 2 => self.clear_row(self.cursor.0),
            _ => {}
        }
    }

    fn parse_param(params: &[u8], default: usize) -> usize {
        if params.is_empty() {
            return default;
        }
        String::from_utf8_lossy(params).parse().unwrap_or(default)
    }

    fn put_cell(&mut self, ch: char) {
        if self.cursor.1 < self.width && self.cursor.0 < self.height {
            self.grid[self.cursor.0][self.cursor.1] = Cell {
                ch,
                style: self.current_style.clone(),
            };
            self.cursor.1 += 1;
        }
    }

    fn clear_row(&mut self, row: usize) {
        if row < self.height {
            for cell in &mut self.grid[row] {
                *cell = Cell { ch: ' ', style: CellStyle::default() };
            }
        }
    }

    fn scroll_up(&mut self) {
        self.grid.remove(0);
        self.grid.push(vec![Cell { ch: ' ', style: CellStyle::default() }; self.width]);
    }

    fn into_screen(self) -> Screen<'static> {
        Screen {
            grid: std::boxed::Box::leak(self.grid.into_boxed_slice()).to_vec(),
            width: self.width,
            height: self.height,
            cursor: self.cursor,
        }
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add src/core/terminal/simulator.rs
git commit -m "feat(tui): implement ANSI parser"
```

---

## Task 3: 实现 TerminalSimulator 结构体

**Files:**
- Modify: `src/core/terminal/simulator.rs`

- [ ] **Step 1: 添加 TerminalSimulator 结构体**

在 `simulator.rs` 末尾添加：

```rust
use crate::tui::input::InputMode;
use crate::tui::terminal::LineEditor;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

/// 终端模拟器
pub struct TerminalSimulator {
    width: usize,
    height: usize,
    parser: AnsiParser,
    live_region: LiveRegion,
    line_editor: LineEditor,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    busy: bool,
}

impl TerminalSimulator {
    /// 创建新的模拟器
    pub fn new(width: usize, height: usize) -> Self {
        let size = TermSize { cols: width as u16, rows: height as u16 };
        TerminalSimulator {
            width,
            height,
            parser: AnsiParser::new(width, height),
            live_region: LiveRegion::new(size),
            line_editor: LineEditor::new(),
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            busy: false,
        }
    }

    /// 发送按键事件
    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self {
        use crate::tui::terminal::EditAction;

        // 处理光标导航
        match key {
            KeyEvent::Up | KeyEvent::Down | KeyEvent::Left | KeyEvent::Right => {
                if let Some(menu) = self.live_region.permission_menu_mut() {
                    match key {
                        KeyEvent::Up => menu.move_up(),
                        KeyEvent::Down => menu.move_down(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        let action = self.line_editor.handle_key(key);
        match action {
            EditAction::Submit(content) => {
                if !content.trim().is_empty() {
                    let text = format!("> {}", content);
                    let _ = self.live_region.commit_text(&mut self.backend(), &text);
                }
            }
            EditAction::Exit => {
                // 处理退出
            }
            _ => {}
        }

        self.render();
        self
    }

    /// 发送 Agent 消息
    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model;
                // 渲染 WelcomeWidget
                let welcome = crate::tui::welcome::WelcomeWidget::new(
                    Some(&self.model_name),
                    "~",
                    None,
                );
                let text = welcome.as_scrollback_string(self.width as u16);
                self.parser.feed(text.as_bytes());
            }
            AgentMessage::Thinking => {
                self.busy = true;
            }
            AgentMessage::TextChunk(s) => {
                let mut backend = TestBackend::new(self.width as u16, self.height as u16);
                let ctx = self.make_status_ctx();
                let _ = self.live_region.frame(&mut backend, "", 0, InputMode::Chat, &ctx);
                self.parser.feed(&backend.output);
            }
            AgentMessage::ToolStart { name, input } => {
                let mut backend = TestBackend::new(self.width as u16, self.height as u16);
                let ctx = self.make_status_ctx();
                let _ = self.live_region.frame(&mut backend, "", 0, InputMode::Chat, &ctx);
                self.parser.feed(&backend.output);
            }
            AgentMessage::ToolEnd { output, .. } => {
                let mut backend = TestBackend::new(self.width as u16, self.height as u16);
                let ctx = self.make_status_ctx();
                let _ = self.live_region.frame(&mut backend, "", 0, InputMode::Chat, &ctx);
                self.parser.feed(&backend.output);
            }
            AgentMessage::PermissionRequest { tool, input } => {
                use crate::tui::live_region::LiveBlock;
                use crate::tui::permission::PermissionState;
                self.live_region.push_live_block(LiveBlock::PermissionPrompt {
                    tool,
                    input,
                    menu: PermissionState::new(),
                });
                self.render();
            }
            AgentMessage::Status(s) => {
                let mut backend = TestBackend::new(self.width as u16, self.height as u16);
                let _ = self.live_region.commit_text(&mut backend, &s);
                self.parser.feed(&backend.output);
            }
            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }
            AgentMessage::Done => {
                self.busy = false;
                self.render();
            }
            AgentMessage::Error(e) => {
                let msg = format!("\u{25cf} error: {}", e);
                let mut backend = TestBackend::new(self.width as u16, self.height as u16);
                let _ = self.live_region.commit_text(&mut backend, &msg);
                self.parser.feed(&backend.output);
            }
            _ => {}
        }
        self
    }

    /// 调整终端大小
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.parser = AnsiParser::new(width, height);
        self.live_region.resize(TermSize { cols: width as u16, rows: height as u16 });
        self.render();
    }

    /// 获取当前画面
    pub fn screen(&self) -> Screen<'_> {
        Screen {
            grid: &self.parser.grid,
            width: self.parser.width,
            height: self.parser.height,
            cursor: self.parser.cursor,
        }
    }

    fn make_status_ctx(&self) -> StatusContext {
        StatusContext {
            cwd: "~".into(),
            branch: None,
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            spinner_frame: if self.busy { Some('|') } else { None },
            spinner_verb: String::new(),
        }
    }

    fn backend(&self) -> TestBackend {
        TestBackend::new(self.width as u16, self.height as u16)
    }

    fn render(&mut self) {
        let mut backend = TestBackend::new(self.width as u16, self.height as u16);
        let ctx = self.make_status_ctx();
        let editor = self.line_editor.content();
        let offset = self.line_editor.cursor_offset();
        let mode = self.line_editor.mode;

        let _ = self.live_region.frame(&mut backend, &editor, offset, mode, &ctx);
        self.parser.feed(&backend.output);
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add src/core/terminal/simulator.rs
git commit -m "feat(tui): implement TerminalSimulator struct"
```

---

## Task 4: 导出模块

**Files:**
- Modify: `src/core/terminal/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: 在 mod.rs 中添加导出**

```rust
pub mod backend;
pub mod buffer;
pub mod events;
pub mod input;
pub mod output;
#[cfg(unix)]
pub mod raw_mode;
pub mod screen;
pub mod simulator;  // 新增
pub mod size;
pub mod style;
```

- [ ] **Step 2: 在测试 mod.rs 中添加**

```rust
mod ansi_serialize_test;
mod block_test;
mod code_block_test;
mod content_test;
mod header_test;
mod inline_flow_test;
mod input_mode_test;
mod input_test;
mod lang_profiles_test;
mod layout_test;
mod live_region_test;
mod markdown_test;
mod message_style_test;
mod paragraph_test;
mod permission_test;
mod qrcode;
mod qrcode_test;
mod renderer_test;
mod simulator_test;  // 新增
mod spinner_test;
mod status_test;
mod syntax_test;
mod terminal_test;
mod tool_call_test;
mod welcome_test;
mod widget_test;
```

- [ ] **Step 3: 提交**

```bash
git add src/core/terminal/mod.rs tests/tui/mod.rs
git commit -m "feat(tui): export simulator module"
```

---

## Task 5: 编写测试

**Files:**
- Create: `tests/tui/simulator_test.rs`

- [ ] **Step 1: 编写基础测试**

```rust
use viv::core::terminal::simulator::{TerminalSimulator, Screen};
use viv::core::terminal::input::KeyEvent;
use viv::agent::protocol::AgentMessage;

#[test]
fn new_simulator_has_empty_screen() {
    let sim = TerminalSimulator::new(80, 24);
    let screen = sim.screen();
    assert_eq!(screen.width, 80);
    assert_eq!(screen.height, 24);
}

#[test]
fn resize_changes_dimensions() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.resize(120, 40);
    let screen = sim.screen();
    assert_eq!(screen.width, 120);
    assert_eq!(screen.height, 40);
}

#[test]
fn send_ready_message_shows_welcome() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "claude-3".into() });
    let screen = sim.screen();
    assert!(screen.contains("claude-3"));
}

#[test]
fn permission_request_shows_menu() {
    let mut sim = TerminalSimulator::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });
    let screen = sim.screen();
    assert!(screen.contains("Bash"));
    assert!(screen.contains("Deny"));
}

#[test]
fn arrow_keys_navigate_permission_menu() {
    let mut sim = TerminalSimulator::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "ls".into(),
    });

    // 初始状态：第一个选项被选中
    let screen = sim.screen();
    assert!(screen.contains("Deny"));

    // 按下键
    sim.send_key(KeyEvent::Down);
    let screen = sim.screen();
    assert!(screen.contains("Allow"));

    // 按上键回到第一个
    sim.send_key(KeyEvent::Up);
    let screen = sim.screen();
    assert!(screen.contains("Deny"));
}

#[test]
fn typing_in_editor_appears_on_screen() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));

    let screen = sim.screen();
    assert!(screen.contains("hi"));
}

#[test]
fn enter_submits_input() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Enter);

    let screen = sim.screen();
    assert!(screen.contains("> hi"));
}
```

- [ ] **Step 2: 运行测试验证**

```bash
cd /data/dlab/viv && cargo test --test simulator_test 2>&1 | head -100
```

- [ ] **Step 3: 修复编译错误（预期会有一些 API 不匹配问题）**

根据错误信息调整实现...

- [ ] **Step 4: 提交**

```bash
git add tests/tui/simulator_test.rs
git commit -m "test(tui): add TerminalSimulator tests"
```

---

## Task 6: 迭代完善

根据测试运行结果，修复任何问题，确保：

1. 所有测试通过
2. API 设计合理
3. 代码质量符合项目标准

---

## 自检清单

- [ ] 设计文档覆盖完整
- [ ] 所有 ANSI 序列正确处理
- [ ] 所有 AgentMessage 类型支持
- [ ] resize 功能正常
- [ ] 键盘导航功能正常
- [ ] 测试覆盖核心场景
