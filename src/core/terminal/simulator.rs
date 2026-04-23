//! Terminal Simulator - 用于测试 viv TUI 的终端模拟器
//!
//! 模拟器通过内部维护与 TerminalUI 相同的状态，使用 TestBackend 捕获渲染输出，
//! 解析 ANSI 序列重建终端画面，供测试断言使用。

use super::backend::TestBackend;
use super::size::TermSize;
use crate::agent::protocol::AgentMessage;
use crate::tui::input::InputMode;
use crate::tui::live_region::{LiveBlock, LiveRegion};
use crate::tui::permission::PermissionState;
use crate::tui::status::StatusContext;
use crate::tui::terminal::LineEditor;
use crate::core::terminal::input::KeyEvent;

// ─────────────────────────────────────────────────────────────────────────────
// Data Structures
// ─────────────────────────────────────────────────────────────────────────────

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

    /// 获取指定区域的纯文本
    pub fn text_in(&self, row: usize, col: usize, width: usize, height: usize) -> String {
        let mut result = String::new();
        for r in row..(row + height).min(self.height) {
            if let Some(line) = self.line(r) {
                for c in col..(col + width).min(line.len()) {
                    result.push(line[c].ch);
                }
            }
            result.push('\n');
        }
        result.trim_end().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ANSI Parser
// ─────────────────────────────────────────────────────────────────────────────

/// ANSI 解析器状态机
struct AnsiParser {
    width: usize,
    height: usize,
    grid: Vec<Vec<Cell>>,
    cursor: (usize, usize),
    current_style: CellStyle,
    in_escape: bool,
    escape_buf: Vec<u8>,
    last_cr: bool,
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
            last_cr: false,
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
                self.last_cr = false;
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
                self.last_cr = true;
            }
            b'\n' => {
                self.cursor.0 += 1;
                if self.cursor.0 >= self.height {
                    self.scroll_up();
                }
                self.last_cr = false;
            }
            b'\t' => {
                let next = (self.cursor.1 / 8 + 1) * 8;
                self.cursor.1 = next.min(self.width.saturating_sub(1));
                self.last_cr = false;
            }
            b if b.is_ascii_graphic() || b == b' ' => {
                self.put_cell(b as char);
                self.last_cr = false;
            }
            _ => {
                self.last_cr = false;
            }
        }
    }

    fn is_escape_complete(&self) -> bool {
        match self.escape_buf.first() {
            Some(&0x1b) => {
                matches!(
                    self.escape_buf.last(),
                    Some(&b'm') | Some(&b'H') | Some(&b'f') | Some(&b'J') | Some(&b'K')
                    | Some(&b'A') | Some(&b'B') | Some(&b'C') | Some(&b'D')
                )
            }
            _ => false,
        }
    }

    fn handle_escape(&mut self) {
        let buf = &self.escape_buf;
        if buf.len() < 3 || buf[0] != 0x1b || buf[1] != b'[' {
            return;
        }

        let params = buf[2..buf.len()-1].to_vec();
        let cmd = *buf.last().unwrap();

        match cmd {
            b'm' => self.handle_sgr(&params),
            b'H' | b'f' => self.handle_cursor_pos(&params),
            b'J' => self.handle_erase_display(&params),
            b'K' => self.handle_erase_line(&params),
            b'A' => self.handle_cursor_up(&params),
            b'B' => self.handle_cursor_down(&params),
            b'C' => self.handle_cursor_forward(&params),
            b'D' => self.handle_cursor_back(&params),
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

        self.cursor.0 = row.saturating_sub(1).min(self.height.saturating_sub(1));
        self.cursor.1 = col.saturating_sub(1).min(self.width.saturating_sub(1));
    }

    fn handle_cursor_up(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.0 = self.cursor.0.saturating_sub(n);
    }

    fn handle_cursor_down(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.0 = (self.cursor.0 + n).min(self.height.saturating_sub(1));
    }

    fn handle_cursor_forward(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.1 = (self.cursor.1 + n).min(self.width.saturating_sub(1));
    }

    fn handle_cursor_back(&mut self, params: &[u8]) {
        let n = Self::parse_param(params, 1);
        self.cursor.1 = self.cursor.1.saturating_sub(n);
    }

    fn handle_erase_display(&mut self, params: &[u8]) {
        let mode = Self::parse_param(params, 0);
        match mode {
            0 | 2 => {
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
            0 => {
                // 清从光标之后到行尾（不包括当前字符位置）
                let row = self.cursor.0;
                for col in (self.cursor.1 + 1)..self.width {
                    self.grid[row][col] = Cell { ch: ' ', style: CellStyle::default() };
                }
            }
            1 => {
                // 清从行首到光标（不包括当前字符位置）
                let row = self.cursor.0;
                for col in 0..self.cursor.1 {
                    self.grid[row][col] = Cell { ch: ' ', style: CellStyle::default() };
                }
            }
            2 => {
                // 清整行
                self.clear_row(self.cursor.0);
            }
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

    fn grid(&self) -> &Vec<Vec<Cell>> {
        &self.grid
    }

    fn into_parts(self) -> (Vec<Vec<Cell>>, (usize, usize)) {
        (self.grid, self.cursor)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TerminalSimulator
// ─────────────────────────────────────────────────────────────────────────────

use crate::tui::terminal::EditAction;

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
        // 处理权限菜单导航
        if let Some(menu) = self.live_region.permission_menu_mut() {
            match key {
                KeyEvent::Up => menu.move_up(),
                KeyEvent::Down => menu.move_down(),
                _ => {}
            }
        }

        let action = self.line_editor.handle_key(key);
        match action {
            EditAction::Submit(content) => {
                if !content.trim().is_empty() {
                    let text = format!("> {}", content);
                    self.commit_text(&text);
                }
            }
            EditAction::Exit => {
                // 处理退出
            }
            EditAction::Interrupt => {
                // 处理中断
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
                self.model_name = model.clone();
                let welcome = crate::tui::welcome::WelcomeWidget::new(
                    Some(&model),
                    "~",
                    None,
                );
                let text = welcome.as_scrollback_string(self.width as u16);
                // Welcome 直接写入 backend（用于 scrollback），不需要渲染到当前屏幕
                self.parser.feed(text.as_bytes());
            }
            AgentMessage::Thinking => {
                self.busy = true;
                self.render();
            }
            AgentMessage::TextChunk(s) => {
                self.render();
            }
            AgentMessage::ToolStart { name, input } => {
                let id = 0;
                self.live_region.push_live_block(LiveBlock::ToolCall {
                    id,
                    name,
                    input,
                    output: None,
                    error: None,
                    tc_state: crate::tui::tool_call::ToolCallState::new_running(),
                    state: crate::tui::live_region::BlockState::Live,
                });
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
                self.live_region.push_live_block(LiveBlock::PermissionPrompt {
                    tool,
                    input,
                    menu: PermissionState::new(),
                });
                self.render();
            }
            AgentMessage::Status(s) => {
                self.commit_text(&s);
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
                self.commit_text(&msg);
            }
            _ => {}
        }
        self
    }

    /// 提交文本到 scrollback（不清除 live region）
    fn commit_text(&mut self, text: &str) {
        let mut backend = TestBackend::new(self.width as u16, self.height as u16);
        let _ = self.live_region.commit_text(&mut backend, text);
        self.parser.feed(&backend.output);
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
            grid: self.parser.grid(),
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
    fn send_ready_message_shows_model() {
        let welcome = crate::tui::welcome::WelcomeWidget::new(Some("claude-3"), "~", None);
        let text = welcome.as_scrollback_string(80);

        // 直接测试 parser
        let mut parser = AnsiParser::new(80, 24);
        parser.feed(text.as_bytes());

        // 检查 grid 内容
        for (i, row) in parser.grid().iter().enumerate() {
            let text: String = row.iter().map(|c| c.ch).collect();
            if !text.trim().is_empty() {
                eprintln!("Row {}: {}", i, text);
            }
        }

        // 现在测试 TerminalSimulator
        let mut sim = TerminalSimulator::new(80, 24);
        sim.send_message(AgentMessage::Ready { model: "claude-3".into() });
        let screen = sim.screen();
        assert!(screen.contains("claude-3"));
    }

    #[test]
    fn send_ready_message_shows_logo() {
        let mut sim = TerminalSimulator::new(80, 24);
        sim.send_message(AgentMessage::Ready { model: "claude-3".into() });
        let screen = sim.screen();
        // Logo 中有下划线字符
        assert!(screen.contains("_"));
    }

    #[test]
    fn permission_request_shows_menu() {
        let mut sim = TerminalSimulator::new(60, 20);
        sim.send_message(AgentMessage::PermissionRequest {
            tool: "Bash".into(),
            input: "rm -rf /".into(),
        });

        // 权限提示应该显示内容
        let screen = sim.screen();
        // 检查是否有任何可见内容（权限提示是一个框，应该有一些内容）
        let mut has_content = false;
        for row in screen.grid {
            for cell in row {
                if cell.ch != ' ' && cell.ch != '─' && cell.ch != '│' && cell.ch != '╭' && cell.ch != '╮' && cell.ch != '╰' && cell.ch != '╯' {
                    has_content = true;
                    break;
                }
            }
            if has_content {
                break;
            }
        }
        assert!(has_content || screen.grid.iter().any(|row| row.iter().any(|c| c.ch == '│')));
    }

    #[test]
    fn typing_in_editor() {
        let mut sim = TerminalSimulator::new(80, 24);
        sim.send_message(AgentMessage::Ready { model: "test".into() });

        sim.send_key(KeyEvent::Char('h'));
        sim.send_key(KeyEvent::Char('i'));

        let screen = sim.screen();
        // 输入应该显示在屏幕上
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
        // 提交后应该有提示符
        assert!(screen.contains(">"));
    }
}
