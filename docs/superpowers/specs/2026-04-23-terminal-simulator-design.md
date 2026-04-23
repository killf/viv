# TerminalSimulator 设计文档

## 概述

TerminalSimulator 是一个用于测试 viv TUI 的终端模拟器。它模拟终端的输入（stdin）和输出（stdout），通过解析 ANSI 字节流来重建终端画面，供测试断言使用。

## 核心数据结构

### CellStyle

```rust
#[derive(Clone, PartialEq, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}
```

### Cell

```rust
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}
```

### Screen

```rust
pub struct Screen<'a> {
    pub grid: &'a Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub cursor: (usize, usize),
}
```

## TerminalSimulator API

```rust
pub struct TerminalSimulator {
    width: usize,
    height: usize,
    grid: Vec<Vec<Cell>>,
    cursor: (usize, usize),
    current_style: CellStyle,
}

impl TerminalSimulator {
    /// 创建新的模拟器
    pub fn new(width: usize, height: usize) -> Self;

    /// 发送按键事件
    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self;

    /// 发送 Agent 消息
    pub fn send_message(&mut self, msg: AgentMessage);

    /// 调整终端大小
    pub fn resize(&mut self, width: usize, height: usize);

    /// 获取当前画面
    pub fn screen(&self) -> &Screen;
}
```

## Screen 断言方法

```rust
impl Screen<'_> {
    /// 获取指定位置的单元格
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell>;

    /// 获取指定行
    pub fn line(&self, row: usize) -> &[Cell];

    /// 检查画面是否包含文本（忽略样式）
    pub fn contains(&self, text: &str) -> bool;

    /// 获取指定区域的文本（忽略样式）
    pub fn text_in(&self, row: usize, col: usize, width: usize, height: usize) -> String;
}
```

## 支持的 ANSI 序列

| 序列 | 处理 |
|------|------|
| `\x1b[row;colH` | 移动光标（1-indexed 转 0-indexed） |
| `\x1b[{n}A` | 光标上移 n 行 |
| `\x1b[{n}B` | 光标下移 n 行 |
| `\x1b[{n}C` | 光标右移 n 列 |
| `\x1b[{n}D` | 光标左移 n 列 |
| `\x1b[2K` | 清整行 |
| `\x1b[K` | 清从光标到行尾 |
| `\x1b[0J` | 清从光标到行尾 |
| `\x1b[J` | 清从光标到屏幕结尾 |
| `\x1b[{n}m` | SGR 颜色/样式 |
| `\x1b[38;2;r;g;bm` | RGB 前景色 |
| `\x1b[48;2;r;g;bm` | RGB 背景色 |
| `\x1b[0m` | 重置样式 |
| `\r` | 光标移到行首 |
| `\n` | 光标下移一行（必要时滚动） |
| `\t` | 光标右移到下一个 8 的倍数 |
| 可见 ASCII 字符 | 写入当前光标位置，光标右移 |

## 内部渲染流程

当 `send_message()` 被调用时，模拟器将 AgentMessage 转换为对应的渲染调用：

```
AgentMessage::Ready -> WelcomeWidget -> backend.write()
AgentMessage::TextChunk -> parse_buffer -> live_region.push_live_block()
AgentMessage::ToolStart -> live_region.push_live_block(ToolCall)
AgentMessage::ToolEnd -> live_region.finish_last_running_tool()
AgentMessage::PermissionRequest -> live_region.push_live_block(PermissionPrompt)
AgentMessage::Status -> live_region.commit_text()
AgentMessage::Tokens -> 更新 StatusContext
AgentMessage::Done -> parse_buffer.flush() -> live_region.drop_trailing_live_markdown()
```

模拟器内部维护与 TerminalUI 相同的状态，使用 TestBackend 捕获渲染输出，然后解析 ANSI 序列来更新内部 grid。

## 位置

`src/core/terminal/simulator.rs`

## 测试示例

```rust
#[test]
fn permission_menu_navigation() {
    let mut sim = TerminalSimulator::new(60, 20);

    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });

    // 初始状态：第一个选项（Deny）被选中
    let screen = sim.screen();
    assert!(screen.contains("Deny"));
    assert!(screen.contains("Bash"));
    assert!(screen.contains("rm -rf /"));

    // 按下键移动到 Allow
    sim.send_key(KeyEvent::Down);
    let screen = sim.screen();
    assert!(screen.contains("Allow"));

    // 按回车确认
    sim.send_key(KeyEvent::Enter);
    // 验证权限请求被处理
}

#[test]
fn resize_terminal() {
    let mut sim = TerminalSimulator::new(80, 24);

    sim.send_message(AgentMessage::Ready { model: "claude".into() });
    assert_eq!(sim.screen().width, 80);
    assert_eq!(sim.screen().height, 24);

    sim.resize(120, 40);
    assert_eq!(sim.screen().width, 120);
    assert_eq!(sim.screen().height, 40);
}

#[test]
fn text_chunk_streaming() {
    let mut sim = TerminalSimulator::new(80, 24);

    sim.send_message(AgentMessage::TextChunk("Hello ".into()));
    let screen = sim.screen();
    assert!(screen.contains("Hello"));

    sim.send_message(AgentMessage::TextChunk("World!".into()));
    let screen = sim.screen();
    assert!(screen.contains("Hello World!"));
}
```

## 后续扩展

- [ ] 支持更多 ANSI 序列（光标保存/恢复、滚动区域等）
- [ ] 支持鼠标事件
- [ ] 画面快照测试（screenshot-like comparison）
- [ ] AI 辅助的画面验证（用 LLM 判断画面是否"正确"）
