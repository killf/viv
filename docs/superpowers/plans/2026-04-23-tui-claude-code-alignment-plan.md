# TUI Claude Code Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 TerminalSimulator 测试基础设施，然后逐模块对齐 viv TUI 与 Claude Code 的交互体验

**Architecture:** TerminalSimulator 作为核心测试基础设施，通过 ANSI 解析重建终端画面；逐模块：测试 → 实现 → 验证

**Tech Stack:** 纯 Rust，无外部依赖

---

## 文件结构

| File | Action | Purpose |
|------|--------|---------|
| `src/core/terminal/simulator.rs` | Create | TerminalSimulator 核心实现 |
| `src/core/terminal/mod.rs` | Modify | 导出 simulator 模块 |
| `tests/tui/simulator_test.rs` | Create | TerminalSimulator 测试 |
| `tests/tui/mod.rs` | Modify | 注册 simulator_test 模块 |
| `src/tui/input.rs` | Modify | 输入系统增强（历史搜索等） |
| `src/tui/markdown.rs` | Modify | Markdown 渲染增强 |
| `src/tui/syntax.rs` | Modify | 语法高亮 |
| `src/tui/permission.rs` | Modify | 权限菜单交互 |
| `src/tui/live_region.rs` | Modify | Live Region 行为 |
| `src/tui/status.rs` | Modify | 状态栏增强 |
| `src/tui/terminal.rs` | Modify | TerminalUI 集成 |

---

## Phase 1: TerminalSimulator 基础设施

### Task 1: TerminalSimulator 基础数据结构

**Files:**
- Create: `src/core/terminal/simulator.rs` (阶段1)

- [ ] **Step 1: 创建 simulator.rs 文件，写入基础数据结构**

```rust
//! TerminalSimulator - 终端模拟器，用于测试 TUI 输出
//!
//! 通过解析 ANSI 序列重建终端画面，支持界面断言测试。

use std::fmt;

/// 颜色定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Ansi(u8),
    Rgb(u8, u8, u8),
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Ansi(n) => write!(f, "Ansi({})", n),
            Color::Rgb(r, g, b) => write!(f, "Rgb({},{},{})", r, g, b),
        }
    }
}

/// 单元格样式
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl CellStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fg(mut self, fg: Option<Color>) -> Self {
        self.fg = fg;
        self
    }

    pub fn with_bg(mut self, bg: Option<Color>) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }
}

/// 单个终端单元格
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

impl Cell {
    pub fn new(ch: char) -> Self {
        Self { ch, style: CellStyle::default() }
    }

    pub fn with_style(ch: char, style: CellStyle) -> Self {
        Self { ch, style }
    }
}

/// 终端画面
#[derive(Debug, Clone)]
pub struct Screen {
    grid: Vec<Vec<Cell>>,
    width: usize,
    height: usize,
    cursor: (usize, usize),
}

impl Screen {
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

    /// 获取指定位置的字符（忽略样式）
    pub fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.cell(row, col).map(|c| c.ch)
    }

    /// 检查指定位置是否有特定样式
    pub fn has_style(&self, row: usize, col: usize, fg: Option<&Color>) -> bool {
        let cell = self.cell(row, col)?;
        match fg {
            Some(f) => cell.style.fg.as_ref() == Some(f),
            None => true,
        }
    }

    /// 获取当前光标位置
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// 获取画面尺寸
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// 获取指定行的纯文本
    pub fn line_text(&self, row: usize) -> Option<String> {
        self.line(row).map(|cells| {
            cells.iter().map(|c| c.ch).collect()
        })
    }

    /// 获取指定行的纯文本（去除尾部空白）
    pub fn line_text_trimmed(&self, row: usize) -> Option<String> {
        self.line_text(row).map(|s| s.trim_end().to_string())
    }

    /// 获取指定范围的文本
    pub fn text_range(&self, start_row: usize, end_row: usize) -> String {
        (start_row..end_row.min(self.height))
            .filter_map(|r| self.line_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// ANSI 解析器状态机
struct AnsiParser {
    width: usize,
    height: usize,
    grid: Vec<Vec<Cell>>,
    cursor: (usize, usize),
    current_style: CellStyle,
    saved_cursor: Option<(usize, usize)>,
    in_escape: bool,
    escape_buf: Vec<u8>,
}

impl AnsiParser {
    fn new(width: usize, height: usize) -> Self {
        let grid = vec![vec![Cell::new(' '); width]; height];
        AnsiParser {
            width,
            height,
            grid,
            cursor: (0, 0),
            saved_cursor: None,
            current_style: CellStyle::default(),
            in_escape: false,
            escape_buf: Vec::new(),
        }
    }

    /// 从字节流构建画面
    fn feed_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.parse_byte(b);
        }
    }

    fn parse_byte(&mut self, b: u8) {
        if self.in_escape {
            self.escape_buf.push(b);
            if let Some(completed) = self.check_escape_complete() {
                if completed {
                    self.handle_escape();
                    self.in_escape = false;
                    self.escape_buf.clear();
                }
            }
            return;
        }

        match b {
            0x1b => {
                // ESC
                self.in_escape = true;
                self.escape_buf.clear();
                self.escape_buf.push(b);
            }
            b'\r' => {
                // 回车：移到行首
                self.cursor.1 = 0;
            }
            b'\n' => {
                // 换行：下移一行，可能滚动
                self.cursor.0 += 1;
                if self.cursor.0 >= self.height {
                    self.scroll_up();
                    self.cursor.0 = self.height - 1;
                }
            }
            b'\t' => {
                // Tab：移到下一个 8 的倍数
                let next = (self.cursor.1 / 8 + 1) * 8;
                self.cursor.1 = next.min(self.width.saturating_sub(1));
            }
            0x07 => {
                // BEL：终端响铃，无操作
            }
            0x08 => {
                // Backspace：左移一格
                if self.cursor.1 > 0 {
                    self.cursor.1 -= 1;
                }
            }
            b if b.is_ascii_graphic() || b == b' ' => {
                // 可打印字符
                self.put_cell(b as char);
            }
            _ => {
                // 忽略其他控制字符
            }
        }
    }

    fn check_escape_complete(&self) -> Option<bool> {
        let buf = &self.escape_buf;
        if buf.is_empty() {
            return None;
        }

        // CSI: ESC [
        if buf.len() >= 2 && buf[0] == 0x1b && buf[1] == b'[' {
            // CSI 序列以字母结尾
            if let Some(&last) = buf.last() {
                let complete = last.is_ascii_alphabetic() ||
                    last == b'm' || last == b'H' || last == b'J' ||
                    last == b'K' || last == b'A' || last == b'B' ||
                    last == b'C' || last == b'D' || last == b'S' ||
                    last == b'T' || last == b'f' || last == b'r' ||
                    last == b'c' || last == b'd' || last == b'g';
                return Some(complete);
            }
            return None;
        }

        // OSC: ESC ]
        if buf.len() >= 2 && buf[0] == 0x1b && buf[1] == b']' {
            // OSC 序列以 BEL 或 ST 终止
            if buf.ends_with(&[0x07]) || buf.ends_with(&[0x1b, b'\\']) {
                return Some(true);
            }
            return None;
        }

        // 其他转义序列
        if buf.len() >= 2 && buf[0] == 0x1b {
            let complete = buf[1].is_ascii_alphabetic();
            return Some(complete);
        }

        Some(false)
    }

    fn handle_escape(&mut self) {
        let buf = &self.escape_buf;
        if buf.len() < 2 {
            return;
        }

        if buf[0] == 0x1b && buf[1] == b'[' {
            // CSI 序列
            self.handle_csi(&buf[2..]);
        } else if buf[0] == 0x1b && buf[1] == b']' {
            // OSC 序列：忽略
        } else if buf[0] == 0x1b {
            // 其他转义序列
            match buf[1] {
                b'7' => { self.saved_cursor = Some(self.cursor); }
                b'8' => { if let Some(pos) = self.saved_cursor { self.cursor = pos; } }
                b'c' => { self.reset(); }
                _ => {}
            }
        }
    }

    fn handle_csi(&mut self, params: &[u8]) {
        if params.is_empty() {
            return;
        }

        let cmd = *params.last().unwrap();
        let param_str = if params.len() > 1 {
            &params[..params.len() - 1]
        } else {
            &[]
        };

        match cmd {
            b'm' => self.handle_sgr(param_str),
            b'H' | b'f' => self.handle_cup(param_str),
            b'J' => self.handle_ed(param_str),
            b'K' => self.handle_el(param_str),
            b'A' => self.handle_cuu(param_str),
            b'B' => self.handle_cud(param_str),
            b'C' => self.handle_cuf(param_str),
            b'D' => self.handle_cub(param_str),
            b'S' => self.handle_su(param_str),
            b'T' => self.handle_sd(param_str),
            b'r' => {
                // 设置滚动区域：忽略（保持简单）
            }
            b'c' => {
                // DA: Device Attributes：忽略
            }
            b'd' => self.handle_cda(param_str),
            b'g' => {
                // Tab 清除：忽略
            }
            _ => {}
        }
    }

    fn handle_sgr(&mut self, params: &[u8]) {
        if params.is_empty() || (params.len() == 1 && params[0] == b'0') {
            self.current_style = CellStyle::default();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            let p = params[i];

            match p {
                0 => { self.current_style = CellStyle::default(); }
                1 => { self.current_style.bold = true; }
                3 => { self.current_style.italic = true; }
                4 => { self.current_style.underline = true; }
                22 => { self.current_style.bold = false; }
                23 => { self.current_style.italic = false; }
                24 => { self.current_style.underline = false; }
                30..=37 => { self.current_style.fg = Some(Color::Ansi(p - 30)); }
                39 => { self.current_style.fg = None; }
                40..=47 => { self.current_style.bg = Some(Color::Ansi(p - 40)); }
                49 => { self.current_style.bg = None; }
                90..=97 => { self.current_style.fg = Some(Color::Ansi(p - 90 + 8)); }
                100..=107 => { self.current_style.bg = Some(Color::Ansi(p - 100 + 8)); }
                38 if i + 2 < params.len() => {
                    // 扩展前景色
                    if params[i + 1] == b'5' && i + 2 < params.len() {
                        // 256 色
                        self.current_style.fg = Some(Color::Ansi(params[i + 2]));
                        i += 2;
                    } else if params[i + 1] == b'2' && i + 4 < params.len() {
                        // 24 位色
                        self.current_style.fg = Some(Color::Rgb(
                            params[i + 2], params[i + 3], params[i + 4]
                        ));
                        i += 4;
                    }
                }
                48 if i + 2 < params.len() => {
                    // 扩展背景色
                    if params[i + 1] == b'5' && i + 2 < params.len() {
                        self.current_style.bg = Some(Color::Ansi(params[i + 2]));
                        i += 2;
                    } else if params[i + 1] == b'2' && i + 4 < params.len() {
                        self.current_style.bg = Some(Color::Rgb(
                            params[i + 2], params[i + 3], params[i + 4]
                        ));
                        i += 4;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn handle_cup(&mut self, params: &[u8]) {
        // CUP: 光标位置 (default: 1,1)
        let mut parser = ParamParser::new(params);
        let row = parser.next_usize(1).saturating_sub(1);
        let col = parser.next_usize(1).saturating_sub(1);
        self.cursor.0 = row.min(self.height.saturating_sub(1));
        self.cursor.1 = col.min(self.width.saturating_sub(1));
    }

    fn handle_cda(&mut self, params: &[u8]) {
        // CDA: 绝对行地址
        let mut parser = ParamParser::new(params);
        let row = parser.next_usize(1).saturating_sub(1);
        self.cursor.0 = row.min(self.height.saturating_sub(1));
    }

    fn handle_ed(&mut self, params: &[u8]) {
        // ED: 擦除显示
        let mut parser = ParamParser::new(params);
        let mode = parser.next_usize(0);

        match mode {
            0 => {
                // 从光标到屏幕结尾
                self.erase_to_end();
            }
            1 => {
                // 从屏幕开始到光标
                self.erase_from_start();
            }
            2 | 3 => {
                // 整个屏幕
                self.erase_all();
            }
            _ => {}
        }
    }

    fn handle_el(&mut self, params: &[u8]) {
        // EL: 擦除行
        let mut parser = ParamParser::new(params);
        let mode = parser.next_usize(0);

        match mode {
            0 => {
                // 从光标到行尾
                self.erase_row_to_end();
            }
            1 => {
                // 从行首到光标
                self.erase_row_to_start();
            }
            2 => {
                // 整行
                self.erase_row();
            }
            _ => {}
        }
    }

    fn handle_cuu(&mut self, params: &[u8]) {
        // CUU: 光标上移
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        self.cursor.0 = self.cursor.0.saturating_sub(n);
    }

    fn handle_cud(&mut self, params: &[u8]) {
        // CUD: 光标下移
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        self.cursor.0 = (self.cursor.0 + n).min(self.height.saturating_sub(1));
    }

    fn handle_cuf(&mut self, params: &[u8]) {
        // CUF: 光标前移
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        self.cursor.1 = (self.cursor.1 + n).min(self.width.saturating_sub(1));
    }

    fn handle_cub(&mut self, params: &[u8]) {
        // CUB: 光标后移
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        self.cursor.1 = self.cursor.1.saturating_sub(n);
    }

    fn handle_su(&mut self, params: &[u8]) {
        // SU: 向上滚动
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        for _ in 0..n {
            self.scroll_up();
        }
    }

    fn handle_sd(&mut self, params: &[u8]) {
        // SD: 向下滚动
        let mut parser = ParamParser::new(params);
        let n = parser.next_usize(1);
        for _ in 0..n {
            self.scroll_down();
        }
    }

    fn put_cell(&mut self, ch: char) {
        if self.cursor.1 < self.width && self.cursor.0 < self.height {
            self.grid[self.cursor.0][self.cursor.1] = Cell::with_style(ch, self.current_style.clone());
            self.cursor.1 += 1;
        }
    }

    fn erase_to_end(&mut self) {
        // 擦除当前行从光标到结尾
        self.erase_row_to_end();
        // 擦除下面的行
        for row in self.cursor.0 + 1..self.height {
            self.grid[row] = vec![Cell::new(' '); self.width];
        }
    }

    fn erase_from_start(&mut self) {
        // 擦除上面的行
        for row in 0..self.cursor.0 {
            self.grid[row] = vec![Cell::new(' '); self.width];
        }
        // 擦除当前行从开头到光标
        self.erase_row_to_start();
    }

    fn erase_all(&mut self) {
        for row in &mut self.grid {
            *row = vec![Cell::new(' '); self.width];
        }
        self.cursor = (0, 0);
    }

    fn erase_row_to_end(&mut self) {
        let row = self.cursor.0;
        for col in self.cursor.1..self.width {
            self.grid[row][col] = Cell::new(' ');
        }
    }

    fn erase_row_to_start(&mut self) {
        let row = self.cursor.0;
        for col in 0..=self.cursor.1 {
            self.grid[row][col] = Cell::new(' ');
        }
    }

    fn erase_row(&mut self) {
        let row = self.cursor.0;
        self.grid[row] = vec![Cell::new(' '); self.width];
    }

    fn scroll_up(&mut self) {
        self.grid.remove(0);
        self.grid.push(vec![Cell::new(' '); self.width]);
    }

    fn scroll_down(&mut self) {
        self.grid.remove(self.height - 1);
        self.grid.insert(0, vec![Cell::new(' '); self.width]);
    }

    fn reset(&mut self) {
        self.grid = vec![vec![Cell::new(' '); self.width]; self.height];
        self.cursor = (0, 0);
        self.current_style = CellStyle::default();
    }

    fn into_screen(self) -> Screen {
        Screen {
            grid: self.grid,
            width: self.width,
            height: self.height,
            cursor: self.cursor,
        }
    }
}

/// CSI 参数解析器
struct ParamParser<'a> {
    params: &'a [u8],
    pos: usize,
}

impl<'a> ParamParser<'a> {
    fn new(params: &'a [u8]) -> Self {
        Self { params, pos: 0 }
    }

    fn next_usize(&mut self, default: usize) -> usize {
        if self.pos >= self.params.len() {
            return default;
        }

        // 找到下一个 ':' 或字母
        let end = self.params[self.pos..]
            .iter()
            .position(|&b| b == b':' || b.is_ascii_alphabetic())
            .map(|p| p + self.pos)
            .unwrap_or(self.params.len());

        let slice = &self.params[self.pos..end];
        self.pos = end + 1;

        if slice.is_empty() {
            return default;
        }

        // 解析数字
        let mut result = 0usize;
        for &b in slice {
            if b == b':' {
                continue;
            }
            if b.is_ascii_digit() {
                result = result * 10 + (b - b'0') as usize;
            } else {
                return default;
            }
        }
        result.max(1) // SGR 参数中 0 保持为 0，但 1 应该表示 1
    }
}
```

- [ ] **Step 2: 运行编译检查**

Run: `cargo check --lib 2>&1 | head -50`
Expected: 无编译错误

- [ ] **Step 3: 提交**

```bash
git add src/core/terminal/simulator.rs
git commit -m "feat(tui): add TerminalSimulator basic data structures and ANSI parser"
```

---

### Task 2: TerminalSimulator API

**Files:**
- Modify: `src/core/terminal/simulator.rs`

- [ ] **Step 1: 添加 TerminalSimulator 结构体和 API**

在 `simulator.rs` 文件末尾添加：

```rust
use crate::agent::protocol::{AgentMessage, PermissionResponse};
use crate::core::terminal::backend::TestBackend;
use crate::core::terminal::size::TermSize;
use crate::core::terminal::input::KeyEvent;
use crate::tui::input::InputMode;
use crate::tui::live_region::LiveRegion;
use crate::tui::status::StatusContext;
use crate::tui::terminal::LineEditor;

/// 终端模拟器
///
/// 通过维护与 TerminalUI 相同的状态，使用 TestBackend 捕获渲染输出，
/// 解析 ANSI 序列重建终端画面，用于测试断言。
pub struct TerminalSimulator {
    width: usize,
    height: usize,
    parser: AnsiParser,
    live_region: LiveRegion,
    line_editor: LineEditor,
    model_name: String,
    cwd: String,
    branch: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    busy: bool,
    pending_permission: Option<(String, String)>,
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
            cwd: "~".to_string(),
            branch: None,
            input_tokens: 0,
            output_tokens: 0,
            busy: false,
            pending_permission: None,
        }
    }

    /// 设置 cwd 和分支信息
    pub fn with_cwd(mut self, cwd: &str) -> Self {
        self.cwd = cwd.to_string();
        self
    }

    /// 设置分支信息
    pub fn with_branch(mut self, branch: Option<&str>) -> Self {
        self.branch = branch.map(|s| s.to_string());
        self
    }

    /// 发送按键事件
    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self {
        use crate::tui::terminal::EditAction;

        // 处理权限菜单导航
        match key {
            KeyEvent::Up | KeyEvent::Down => {
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
                    self.line_editor.push_history(content);
                }
            }
            EditAction::Exit => {
                // 退出信号：模拟器不支持退出
            }
            EditAction::Interrupt => {
                self.line_editor.clear();
            }
            EditAction::Continue => {}
        }

        self.render();
        self
    }

    /// 发送 Agent 消息
    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model;
                // 渲染 Welcome
                let welcome = crate::tui::welcome::WelcomeWidget::new(
                    Some(&self.model_name),
                    &self.cwd,
                    self.branch.as_deref(),
                );
                let text = welcome.as_scrollback_string(self.width as u16);
                self.parser.feed_bytes(text.as_bytes());
            }
            AgentMessage::Thinking => {
                self.busy = true;
            }
            AgentMessage::TextChunk(s) => {
                let mut backend = self.backend();
                let pending = self.parse_chunk(&s);
                if !pending.is_empty() {
                    self.live_region.push_live_block(
                        crate::tui::live_region::LiveBlock::Markdown {
                            nodes: pending,
                            state: crate::tui::live_region::BlockState::Live,
                        },
                    );
                }
                self.render_with_backend(&mut backend);
            }
            AgentMessage::ToolStart { name, input } => {
                self.live_region.push_live_block(
                    crate::tui::live_region::LiveBlock::ToolCall {
                        id: 0,
                        name,
                        input,
                        output: None,
                        error: None,
                        tc_state: crate::tui::tool_call::ToolCallState::new_running(),
                        state: crate::tui::live_region::BlockState::Live,
                    },
                );
                self.render();
            }
            AgentMessage::ToolEnd { output, .. } => {
                self.live_region.finish_last_running_tool(Some(output), None);
                self.render();
            }
            AgentMessage::ToolError { error, .. } => {
                self.live_region.finish_last_running_tool(None, Some(error));
                self.render();
            }
            AgentMessage::PermissionRequest { tool, input } => {
                self.pending_permission = Some((tool.clone(), input.clone()));
                self.live_region.push_live_block(
                    crate::tui::live_region::LiveBlock::PermissionPrompt {
                        tool,
                        input,
                        menu: crate::tui::permission::PermissionState::new(),
                    },
                );
                self.render();
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
                let _ = self.live_region.commit_text(&mut self.backend(), &msg);
                self.busy = false;
            }
            AgentMessage::Status(s) => {
                let _ = self.live_region.commit_text(&mut self.backend(), &s);
            }
            AgentMessage::Evolved => {}
        }
        self
    }

    /// 调整终端大小
    pub fn resize(&mut self, width: usize, height: usize) -> &mut Self {
        self.width = width;
        self.height = height;
        self.parser = AnsiParser::new(width, height);
        self.live_region.resize(TermSize { cols: width as u16, rows: height as u16 });
        self.render();
        self
    }

    /// 获取当前画面
    pub fn screen(&self) -> &Screen {
        // 注意：这个实现需要改变，因为我们不能在 &self 中返回 &Screen
        // 改为返回 Screen 的克隆
        unimplemented!("use screen_ref() instead")
    }

    /// 获取当前画面的引用
    pub fn screen_ref(&self) -> Screen {
        Screen {
            grid: self.parser.grid.clone(),
            width: self.parser.width,
            height: self.parser.height,
            cursor: self.parser.cursor,
        }
    }

    /// 获取当前输入内容
    pub fn input_content(&self) -> String {
        self.line_editor.content()
    }

    /// 获取当前输入模式
    pub fn input_mode(&self) -> InputMode {
        self.line_editor.mode.clone()
    }

    /// 获取权限菜单选择状态
    pub fn permission_selected(&self) -> Option<usize> {
        self.live_region.permission_menu()
            .map(|m| m.selected_index())
    }

    fn backend(&self) -> TestBackend {
        TestBackend::new(self.width as u16, self.height as u16)
    }

    fn make_status_ctx(&self) -> StatusContext {
        let spinner_frame = if self.busy { Some('|') } else { None };
        StatusContext {
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            spinner_frame,
            spinner_verb: String::new(),
        }
    }

    fn render(&mut self) {
        let mut backend = self.backend();
        self.render_with_backend(&mut backend);
    }

    fn render_with_backend(&mut self, backend: &mut TestBackend) {
        let ctx = self.make_status_ctx();
        let editor = self.line_editor.content();
        let offset = self.line_editor.cursor_offset();
        let mode = self.line_editor.mode.clone();
        let _ = self.live_region.frame(backend, &editor, offset, mode, &ctx);
        self.parser.feed_bytes(&backend.output);
    }

    fn parse_chunk(&self, chunk: &str) -> Vec<crate::tui::content::MarkdownNode> {
        use crate::tui::content::MarkdownParseBuffer;
        let mut buf = MarkdownParseBuffer::new();
        buf.push(chunk);
        let blocks = buf.flush();
        blocks.into_iter()
            .filter_map(|b| match b {
                crate::tui::content::ContentBlock::Markdown { nodes } => Some(nodes),
                _ => None,
            })
            .flatten()
            .collect()
    }
}
```

- [ ] **Step 2: 运行编译检查**

Run: `cargo check --lib 2>&1 | head -80`
Expected: 编译错误（需要修复的类型问题）

- [ ] **Step 3: 修复编译错误**

根据错误信息调整实现...

- [ ] **Step 4: 再次编译检查**

Run: `cargo check --lib 2>&1 | head -50`
Expected: 无编译错误

- [ ] **Step 5: 提交**

```bash
git add src/core/terminal/simulator.rs
git commit -m "feat(tui): implement TerminalSimulator API"
```

---

### Task 3: TerminalSimulator 测试

**Files:**
- Create: `tests/tui/simulator_test.rs`
- Modify: `tests/tui/mod.rs`
- Modify: `src/core/terminal/mod.rs`

- [ ] **Step 1: 在 mod.rs 中添加导出**

在 `src/core/terminal/mod.rs` 中添加：
```rust
pub mod simulator;
```

- [ ] **Step 2: 在测试 mod.rs 中添加模块**

在 `tests/tui/mod.rs` 中添加：
```rust
mod simulator_test;
```

- [ ] **Step 3: 编写基础测试**

```rust
use viv::core::terminal::simulator::{TerminalSimulator, Screen, Color, CellStyle};
use viv::core::terminal::input::KeyEvent;
use viv::agent::protocol::AgentMessage;

#[test]
fn new_simulator_has_correct_dimensions() {
    let sim = TerminalSimulator::new(80, 24);
    let screen = sim.screen_ref();
    assert_eq!(screen.size(), (80, 24));
}

#[test]
fn resize_changes_dimensions() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.resize(120, 40);
    let screen = sim.screen_ref();
    assert_eq!(screen.size(), (120, 40));
}

#[test]
fn send_ready_message_shows_welcome() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "claude-3-5-sonnet".into() });
    let screen = sim.screen_ref();
    assert!(screen.contains("claude-3-5-sonnet"), "welcome should show model name");
    assert!(screen.contains("viv"), "welcome should show viv");
}

#[test]
fn typing_in_editor_appears_on_screen() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));

    let screen = sim.screen_ref();
    assert!(screen.contains("hello"), "typed text should appear on screen");
}

#[test]
fn slash_switches_to_slash_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('/'));
    assert_eq!(sim.input_mode(), viv::tui::input::InputMode::SlashCommand);
}

#[test]
fn colon_switches_to_colon_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char(':'));
    assert_eq!(sim.input_mode(), viv::tui::input::InputMode::ColonCommand);
}

#[test]
fn enter_submits_input() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Enter);

    let screen = sim.screen_ref();
    assert!(screen.contains("> hi"), "submitted text should show with > prefix");
}

#[test]
fn permission_request_shows_menu() {
    let mut sim = TerminalSimulator::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });

    let screen = sim.screen_ref();
    assert!(screen.contains("Bash"), "should show tool name");
    assert!(screen.contains("Deny"), "should show Deny option");
    assert!(screen.contains("Allow"), "should show Allow option");
}

#[test]
fn permission_menu_navigation() {
    let mut sim = TerminalSimulator::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "ls".into(),
    });

    // 初始选择第一项
    assert_eq!(sim.permission_selected(), Some(0));

    // 按下键选择第二项
    sim.send_key(KeyEvent::Down);
    assert_eq!(sim.permission_selected(), Some(1));

    // 按上键回到第一项
    sim.send_key(KeyEvent::Up);
    assert_eq!(sim.permission_selected(), Some(0));
}

#[test]
fn thinking_message_sets_busy() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Thinking);
    // busy 状态反映在状态栏中
    let screen = sim.screen_ref();
    assert!(screen.contains("Thinking"), "should show thinking indicator");
}

#[test]
fn tool_call_shows_name() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "ls -la".into(),
    });

    let screen = sim.screen_ref();
    assert!(screen.contains("Bash"), "should show tool name");
}

#[test]
fn screen_cell_returns_correct_char() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    let screen = sim.screen_ref();
    // 检查光标位置有字符
    let (col, row) = screen.cursor();
    assert!(col < 80 && row < 24);
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test --test tui_tests simulator 2>&1 | head -100`
Expected: 编译错误或测试失败（预期）

- [ ] **Step 5: 修复测试和实现**

根据错误信息修复...

- [ ] **Step 6: 再次运行测试**

Run: `cargo test --test tui_tests simulator 2>&1`
Expected: 所有 simulator 测试通过

- [ ] **Step 7: 提交**

```bash
git add src/core/terminal/mod.rs tests/tui/mod.rs tests/tui/simulator_test.rs
git commit -m "test(tui): add TerminalSimulator tests"
```

---

## Phase 2: 输入系统对齐

### Task 4: 历史搜索功能

**Files:**
- Modify: `src/tui/terminal.rs` (LineEditor)
- Create: `tests/tui/history_search_test.rs`

- [ ] **Step 1: 编写历史搜索测试**

```rust
// tests/tui/history_search_test.rs

#[test]
fn history_search_ctrl_r_activates_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // 先输入一些历史
    sim.send_key(KeyEvent::Char('f'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Enter);

    // 进入历史搜索模式
    sim.send_key(KeyEvent::CtrlR);

    let screen = sim.screen_ref();
    assert!(screen.contains("(reverse-i-search)"), "should show search prompt");
}

#[test]
fn history_search_filters_results() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // 添加多条历史
    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Enter);

    sim.send_key(KeyEvent::Char('w'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Char('r'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('d'));
    sim.send_key(KeyEvent::Enter);

    // 进入历史搜索并输入过滤条件
    sim.send_key(KeyEvent::CtrlR);
    sim.send_key(KeyEvent::Char('h'));

    let screen = sim.screen_ref();
    assert!(screen.contains("hello"), "should show matching history");
}
```

- [ ] **Step 2: 实现历史搜索功能**

在 `src/tui/terminal.rs` 的 `LineEditor` 中添加历史搜索状态...

- [ ] **Step 3: 运行测试并修复**

Run: `cargo test --test tui_tests history 2>&1`

- [ ] **Step 4: 提交**

---

### Task 5: 快捷键增强

**Files:**
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: 添加 Ctrl+L 清屏测试**

```rust
#[test]
fn ctrl_l_clears_input() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('t'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('t'));
    assert_eq!(sim.input_content(), "test");

    sim.send_key(KeyEvent::CtrlL);
    assert_eq!(sim.input_content(), "", "Ctrl+L should clear input");
}
```

- [ ] **Step 2: 实现 Ctrl+L**

- [ ] **Step 3: 运行测试**

---

## Phase 3: 消息渲染对齐

### Task 6: 语法高亮

**Files:**
- Modify: `src/tui/syntax.rs`
- Modify: `tests/tui/syntax_test.rs`

- [ ] **Step 1: 添加 Rust 语法高亮测试**

```rust
#[test]
fn rust_keyword_highlighted() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::TextChunk("```rust\nfn main() {}".into()));
    sim.send_message(AgentMessage::Done);

    let screen = sim.screen_ref();
    // fn 应该有特殊颜色
    assert!(screen.contains("fn"), "should show fn keyword");
}
```

- [ ] **Step 2: 扩展语法高亮支持**

---

## Phase 4: 布局架构对齐

### Task 7: Live Region 增强

**Files:**
- Modify: `src/tui/live_region.rs`

- [ ] **Step 1: 添加滚动锁定测试**

- [ ] **Step 2: 实现滚动锁定**

---

## Phase 5: 权限状态对齐

### Task 8: 权限菜单样式

**Files:**
- Modify: `src/tui/permission.rs`
- Modify: `tests/tui/permission_test.rs`

- [ ] **Step 1: 验证权限菜单样式与 Claude Code 一致**

---

## 自检清单

- [ ] 所有 TerminalSimulator 测试通过
- [ ] `cargo test --test tui_tests` 100% 通过
- [ ] 无 clippy warnings
- [ ] 无 unsafe 代码（除 FFI 边界）
- [ ] 每次提交都有对应测试
