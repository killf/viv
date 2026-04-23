# TuiSession 抽取与 SimTerminal 重建设计

**日期**：2026-04-23
**作者**：killf
**状态**：待审阅

## 背景

当前 `src/core/terminal/simulator.rs::TerminalSimulator` 是一个独立于真 UI 的"仿真器"：它自己持有 `LiveRegion`、`LineEditor`、`cwd`、`model_name` 等字段，自己实现 `send_message` / `send_key` / `render`。这份代码和 `src/tui/terminal.rs::TerminalUI` 平行维护，导致：

1. 任何真 UI 逻辑改动都要在 simulator 复制一次，否则测试和真行为漂移。
2. `e2e_welcome_screen_layout` 当前之所以通过，是因为 simulator 在 `AgentMessage::Ready` 后**没**调用 `render()`，input 框根本没画出来 —— 断言里 20 行空白，掩盖了真 UI 会显示的输入框和状态栏。
3. 一旦让 simulator 跑 `render()`，`Screen::write_char` 在最后一行最后一列立即触发 wrap+scroll，把 welcome 顶部内容挤出屏幕，测试立即崩。

长期目标是 Agent 作为应用入口、持有一个 `Terminal`（真实或模拟）。本次只对齐**终端渲染路径**，不动 Agent / main.rs 的线程 + 通道编排。

## 目标

- 抽出共享渲染核心 `TuiSession`，真 UI 和仿真器共用
- 仿真器（改名 `SimTerminal`）通过 `TuiSession` + `TestBackend` + `AnsiParser` 运行，和真 UI 100% 共享渲染代码
- 修 `Screen::write_char` 的 auto-wrap bug（改 xterm deferred-wrap）
- 重写 `e2e_welcome_screen_layout`：24 行完整文本断言 + logo / 边框 / 状态栏的颜色断言

## 非目标

- 不重构 Agent 持有 Terminal 这一层（日后单独做；本次只让渲染路径收敛）
- 不增加新 e2e 测试；之前被误删的其它 17 个测试**不**恢复（用户明确说"只做 e2e_welcome_screen_layout"）
- 不改 main.rs / Agent / LLM / MCP / LSP

## 架构

```
┌─────────────────────────────────────────────────┐
│  TerminalUI (src/tui/terminal.rs)               │
│    event_tx  msg_rx  backend: CrossBackend      │
│    event_loop  session: TuiSession              │
│    run() { loop { 收 msg → session.handle_msg;  │
│                   poll key → session.handle_key │
│                             → KeyOutcome 分派;  │
│                   render → session.render_frame }│
└──────────────┬──────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────┐
│  TuiSession (src/tui/session.rs, NEW)           │
│    live_region  editor  cwd  branch             │
│    model_name   parse_buffer  tool_seq          │
│    input_tokens output_tokens  busy             │
│    spinner  spinner_start  spinner_verb         │
│    pending_permission  quitting  quitting_start │
│                                                 │
│    handle_message(msg, &mut dyn Backend)        │
│    render_frame(&mut dyn Backend) -> CursorPos  │
│    handle_key(key, &mut dyn Backend) -> KeyOutcome│
│    resize(TermSize)                             │
└──────────────▲──────────────────────────────────┘
               │
┌──────────────┴──────────────────────────────────┐
│  SimTerminal (src/core/terminal/simulator.rs)   │
│    session: TuiSession                          │
│    backend: TestBackend  parser: AnsiParser     │
│    sent_events: Vec<AgentEvent>                 │
│    quit_requested: bool                         │
│                                                 │
│    send_message(msg) { session.handle_message;  │
│                        session.render_frame;    │
│                        parser.parse(backend);   │
│                        backend.output.clear; }  │
│    send_key(k) { 同上 + 收 KeyOutcome }         │
│    screen() -> Screen                           │
└─────────────────────────────────────────────────┘
```

## 组件

### `TuiSession`（`src/tui/session.rs`，新）

```rust
pub struct TuiSession {
    live_region: LiveRegion,
    editor: LineEditor,
    cwd: String,
    branch: Option<String>,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<Instant>,
    spinner_verb: String,
    parse_buffer: MarkdownParseBuffer,
    tool_seq: usize,
    pending_permission: Option<(String, String)>,
    quitting: bool,
    quitting_start: Option<Instant>,
}

pub enum KeyOutcome {
    None,
    Event(AgentEvent),   // Submit / Interrupt / PermissionResponse / Quit
}

impl TuiSession {
    pub fn new(size: TermSize, cwd: String, branch: Option<String>) -> Self;
    pub fn handle_message(&mut self, msg: AgentMessage, backend: &mut dyn Backend) -> Result<()>;
    pub fn render_frame(&mut self, backend: &mut dyn Backend) -> Result<CursorPos>;
    pub fn handle_key(&mut self, key: KeyEvent, backend: &mut dyn Backend) -> Result<KeyOutcome>;
    pub fn resize(&mut self, new_size: TermSize);
    pub fn is_busy(&self) -> bool;
    pub fn is_quitting(&self) -> bool;
    pub fn enter_quitting_mode(&mut self);
    pub fn last_live_rows(&self) -> u16;
    pub fn input_content(&self) -> String;
    pub fn input_mode(&self) -> InputMode;
    pub fn permission_selected(&self) -> Option<usize>;
}
```

`TuiSession` 不持有 backend，不碰事件循环，不碰通道。方法接 `&mut dyn Backend` 以便 `TerminalUI` 传 `CrossBackend`、`SimTerminal` 传 `TestBackend`。

### `TerminalUI`（`src/tui/terminal.rs`，改）

退化为三件事的壳：
1. 拥有真 backend (`CrossBackend`)、`event_tx`、`msg_rx`、`EventLoop`
2. 在主循环里把 `msg_rx` drain 给 `session.handle_message`，把 `event_loop.poll` 的结果给 `session.handle_key`，然后 `KeyOutcome::Event(e)` 走 `event_tx.send(e)`。Ctrl+D 在 `session.handle_key` 内部完成 `quitting = true` + `quitting_start` 设置，并返回 `Event(AgentEvent::Quit)`。
3. `cleanup()` 和 `read_cwd_branch()` 保留

字段从约 16 个降到 5 个：`event_tx`、`msg_rx`、`backend`、`event_loop`、`session`。

### `SimTerminal`（`src/core/terminal/simulator.rs`，改）

```rust
pub struct SimTerminal {
    session: TuiSession,
    backend: TestBackend,
    parser: AnsiParser,
    sent_events: Vec<AgentEvent>,
}

impl SimTerminal {
    pub fn new(width: usize, height: usize) -> Self;
    pub fn with_cwd(self, cwd: &str) -> Self;
    pub fn with_branch(self, branch: Option<&str>) -> Self;
    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self;
    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self;
    pub fn resize(&mut self, width: usize, height: usize) -> &mut Self;
    pub fn screen(&self) -> Screen;
    pub fn input_content(&self) -> String;
    pub fn input_mode(&self) -> InputMode;
    pub fn permission_selected(&self) -> Option<usize>;
    pub fn sent_events(&self) -> &[AgentEvent];
    pub fn quit_requested(&self) -> bool;   // 委托 session.is_quitting()
}
```

**驱动流程**（每次 send 后都走一遍）：
1. `session.handle_message(msg, &mut backend)` 或 `session.handle_key(key, &mut backend)`
2. 若 `handle_key` 返回 `KeyOutcome::Event(e)`，`sent_events.push(e)`（Quit 以 `Event(AgentEvent::Quit)` 的形式出现；`session.is_quitting()` 已同步置 true）
3. `session.render_frame(&mut backend)`
4. `parser.parse(&backend.output); backend.output.clear();`

### ANSI 解析修复（`Screen` + `AnsiParser`）

- `Screen` 加字段 `pending_wrap: bool`
- `write_char`：写完字符后若 `cursor.1 + 1 >= width`，不立即 wrap，只置 `pending_wrap = true`，cursor 停在最后一列；下次写字符时若 `pending_wrap`，先换行（必要时 scroll）再写、清 flag
- `move_cursor_to`、`move_cursor_rel`（CUP / CUU / CUD / CUF / CUB 的底座）：清 `pending_wrap`
- parser ground 态的 `\r` 分支：清 `pending_wrap`
- parser ground 态的 `\n` 分支：清 `pending_wrap`（下一行正常）
- `scroll` 自身不清，因为可能在 LF 的滚动里也要处理 pending

这是独立的真 bug：当前代码在最后一行最后一列立即触发换行+滚屏，违背 xterm 的 Last Column Flag 行为。

### 新断言（`Screen`）

```rust
pub fn assert_cell_fg_rgb(&self, row: usize, col: usize, r: u8, g: u8, b: u8);
```

旧的 `assert_cell_fg(row, col, u8)` 保留，覆盖 ANSI 30-37/90-97 场景。

## 数据流

1. 测试构造 `SimTerminal::new(80, 24).with_cwd("/data/project")`
2. 测试 `sim.send_message(AgentMessage::Ready { model: "...".into() })`
3. `SimTerminal::send_message` →
   - `session.handle_message` 里 `AgentMessage::Ready` 分支：set `model_name`，构 `WelcomeWidget`，`welcome.as_scrollback_string` 写进 backend
   - `session.render_frame` → `live_region.frame` → backend（输入框 + 状态栏）
   - `parser.parse(&backend.output)` → 更新 `Screen`
   - `backend.output.clear()`
4. 测试 `sim.screen().assert_screen(&[r"...", r"...", ...])` + 一串 `assert_cell_fg_rgb`

## 错误处理

- `TuiSession` 所有方法返回 `Result<...>`；`SimTerminal::send_*` 内部 `?` 冒泡，失败时 `sim.last_error()` 留作 v2。本轮：方法里遇错直接 `panic!`，因为测试环境不会失败（`TestBackend` 所有 I/O 返回 `Ok`，`render` 逻辑不涉及网络）。

## 测试策略

**本轮唯一新增测试**：`tests/tui/e2e_screen_test.rs::e2e_welcome_screen_layout`

```rust
#[test]
fn e2e_welcome_screen_layout() {
    std::env::set_var("SHELL", "/bin/zsh");  // 固定 $SHELL
    let mut sim = SimTerminal::new(80, 24).with_cwd("/data/project");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });
    let screen = sim.screen();

    // 完整 24 行文本
    screen.assert_screen(&[
        r"       _           Model:    claude-3-5-sonnet-20241022",
        r"__   _(_)_   __    CWD:      /data/project",
        r"\ \ / / \ \ / /    Branch:   -",
        r" \ V /| |\ V /     Platform: linux x86_64",
        r"  \_/ |_| \_/      Shell:    zsh",
        "", "", "", "", "",
        "", "", "", "", "", "", "", "", "", "",
        "────────────────────────────────────────────────────────────────────────────────",
        "❯",
        "────────────────────────────────────────────────────────────────────────────────",
        r"  /data/project                    claude-3-5-sonnet-20241022  ↑ 0  ↓ 0  ~$0.000",
    ]);

    // 颜色断言（CLAUDE=RGB(215,119,87), DIM=RGB(136,136,136), TEXT=RGB(255,255,255)）
    screen.assert_cell_fg_rgb(0, 7, 215, 119, 87);       // logo '_'
    screen.assert_cell_fg_rgb(2, 0, 215, 119, 87);       // logo '\'
    screen.assert_cell_fg_rgb(0, 19, 215, 119, 87);      // Model 标签 'M'
    screen.assert_cell_fg_rgb(0, 29, 255, 255, 255);     // model 值 'c'
    screen.assert_cell_fg_rgb(1, 29, 255, 255, 255);     // cwd 值 '/'
    screen.assert_cell_fg_rgb(20, 0, 136, 136, 136);     // 顶边 '─'
    screen.assert_cell_fg_rgb(22, 79, 136, 136, 136);    // 底边末尾 '─'
    screen.assert_cell_fg_rgb(21, 0, 215, 119, 87);      // prompt ❯
    screen.assert_cell_fg_rgb(23, 2, 136, 136, 136);     // 状态栏 cwd '/'
    screen.assert_cell_fg_rgb(23, 35, 136, 136, 136);    // 状态栏 model 'c'
}
```

## 风险 & 缓解

1. **`$SHELL` 环境依赖**：测试首行 `std::env::set_var("SHELL", "/bin/zsh")` 强制固定。生产代码不受影响。
2. **`TuiSession::handle_key` 的 `commit_text` / `drop_permission_prompt` 写 backend**：`handle_key` 签名必须收 `&mut dyn Backend`，已在接口中体现。
3. **spinner 依赖 `Instant::now()`**：只要测试不发 `AgentMessage::Thinking`，`busy=false`、`spinner_start=None`，render 时 `spinner_frame=None`，状态栏不带 spinner。本测试不发 Thinking，安全。
4. **滚屏 bug 修复可能影响其它测试**：保留的 ANSI 解析单元测试（`test_parse_cup` 等）都在非边界位置断言，不受 pending_wrap 影响。`cargo test` 全量通过作为验收标准。

## 验收

```
cargo build              # 通过
cargo test               # 通过（含修完的 e2e_welcome_screen_layout）
cargo clippy             # 通过
```

## 后续（非本轮）

- `Terminal` trait 形成：`trait Terminal { fn send(&mut self, msg: AgentMessage); fn next_event(&mut self) -> AgentEvent; }`
- `RealTerminal`（包 `TerminalUI`）和 `SimTerminal` 实现该 trait
- `Agent::new(config, terminal: Box<dyn Terminal>)`、`agent.run()` 消费 terminal；`main.rs` 简化为 `Agent::new(config, RealTerminal::new()?).run()`
- 至此完成"Agent 是整个应用的入口"的架构目标
