# TUI Claude Code 完整对齐设计方案

**Goal:** 让 viv 的 TUI 与 Claude Code 在输入系统、消息渲染、布局架构、权限状态四个方面完全一致

**Architecture:**
- TerminalSimulator 作为核心测试基础设施
- 逐模块：测试 → 实现 → 验证
- 每次提交必须附带对应测试

**Tech Stack:** 纯 Rust，无外部依赖

---

## 文件结构

```
src/
├── core/terminal/
│   ├── simulator.rs    # TerminalSimulator 核心
│   └── mod.rs
├── tui/
│   ├── input.rs       # 多行输入 + 历史搜索
│   ├── markdown.rs    # Markdown 渲染增强
│   ├── syntax.rs      # 语法高亮
│   ├── tool_call.rs   # 工具调用显示
│   └── permission.rs  # 权限提示菜单
└── agent/
    └── protocol.rs    # AgentMessage 定义

tests/tui/
├── simulator_test.rs
├── input_test.rs
├── markdown_test.rs
├── syntax_test.rs
├── tool_call_test.rs
└── permission_test.rs
```

---

## Phase 1: TerminalSimulator 基础设施

### 1.1 数据结构

```rust
// 颜色定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Ansi(u8),
    Rgb(u8, u8, u8),
}

// 单元格样式
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
}

// 单个单元格
#[derive(Debug, Clone)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

// 终端画面
#[derive(Debug, Clone)]
pub struct Screen {
    pub grid: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub cursor: (usize, usize),
}
```

### 1.2 ANSI 解析器

支持序列：
- SGR: `\x1b[...m` (颜色、粗体、下划线等)
- 光标: `\x1b[H`, `\x1b[{n}A/B/C/D`
- 擦除: `\x1b[J`, `\x1b[K`
- 移动: `\x1b[{n};{n}H`

### 1.3 TerminalSimulator API

```rust
pub struct TerminalSimulator {
    width: usize,
    height: usize,
    parser: AnsiParser,
    live_region: LiveRegion,
    line_editor: LineEditor,
}

impl TerminalSimulator {
    pub fn new(width: usize, height: usize) -> Self;
    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self;
    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self;
    pub fn resize(&mut self, width: usize, height: usize);
    pub fn screen(&self) -> &Screen;
}

impl Screen {
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell>;
    pub fn line(&self, row: usize) -> Option<&[Cell]>;
    pub fn contains(&self, text: &str) -> bool;
    pub fn has_style(&self, row: usize, col: usize, fg: Option<Color>) -> bool;
    pub fn cursor(&self) -> (usize, usize);
}
```

---

## Phase 2: 输入系统对齐

### 2.1 多行编辑

当前实现 vs Claude Code:
- [x] Shift+Enter 插入换行
- [x] Backspace 在行首合并上一行
- [x] Delete 合并下一行
- [ ] 光标上下键在多行间移动
- [ ] Tab 缩进支持

### 2.2 历史搜索

需要实现:
- [ ] Ctrl+R 进入历史搜索模式
- [ ] 输入字符过滤历史
- [ ] 上下键浏览匹配项
- [ ] Enter 选择 / Ctrl+C 退出

### 2.3 快捷键绑定

| 快捷键 | 功能 |
|--------|------|
| Ctrl+C | 取消当前输入 / 中断 Agent |
| Ctrl+D | 空输入时退出 |
| Ctrl+L | 清屏 |
| Ctrl+Z | 挂起 (unix) |
| Tab | 自动补全 (未来) |

### 2.4 输入模式

| 模式 | 提示符 | 行为 |
|------|--------|------|
| Chat | `› ` | 正常输入 |
| Slash | `/ ` | 斜杠命令 |
| Colon | `: ` | 冒号命令 |

---

## Phase 3: 消息渲染对齐

### 3.1 Markdown 渲染

当前支持 vs 需要支持:

| 特性 | 当前 | 目标 |
|------|------|------|
| 粗体 **text** | [x] | [x] |
| 斜体 *text* | [x] | [x] |
| 行内代码 `code` | [x] | [x] |
| 链接 [text](url) | [x] | [x] |
| 标题 # ## ### | [x] | [x] |
| 列表 - * 1. | [x] | [x] |
| 引用 > | [x] | [x] |
| 水平线 --- | [x] | [x] |
| 代码块 ``` | [x] | [x] |
| 语法高亮 | [ ] | [x] |

### 3.2 语法高亮

需要支持语言:
- Rust
- TypeScript/JavaScript
- Python
- Shell/Bash
- JSON/YAML
- Markdown
- Go
- C/C++

### 3.3 消息样式

| 类型 | 前缀 | 颜色 |
|------|------|------|
| 用户消息 | `> ` | Claude orange |
| 助手消息 | `● ` | Claude orange |
| 系统消息 | `  ` | DIM |
| 工具调用 | `⟨name⟩` | DIM |
| 工具结果 | `  ` | TEXT |
| 错误 | `✗ ` | RED |

---

## Phase 4: 布局架构对齐

### 4.1 屏幕布局

```
┌─────────────────────────────────────────┐
│  ~/project  ⎇ main                      │  <- Header (1行)
├─────────────────────────────────────────┤
│                                         │
│  ● 助手消息渲染区域                      │
│  › 用户消息                             │
│  ⟨工具⟩                                │
│                                         │
│  ... scrollback ...                     │
│                                         │
├─────────────────────────────────────────┤
│  ┌─────────────────────────────────────┐│
│  │ › 用户输入区域                       ││  <- Input Box (动态高度)
│  └─────────────────────────────────────┘│
├─────────────────────────────────────────┤
│  claude-opus-4-6  ↑ 1234  ↓ 5678  ~$0 │  <- Status Bar (1行)
└─────────────────────────────────────────┘
```

### 4.2 Live Region 行为

- [ ] 消息流式传输时实时更新
- [ ] 工具调用开始/结束动画
- [ ] 权限提示菜单覆盖
- [ ] 滚动锁定（新消息时保持底部）

### 4.3 光标管理

- [ ] 光标位置精确计算
- [ ] 多行输入时光标移动
- [ ] 权限菜单选择指示器

---

## Phase 5: 权限和状态对齐

### 5.1 权限提示菜单

```rust
pub enum PermissionOption {
    Deny,         // 拒绝
    Allow,        // 允许一次
    AlwaysAllow,  // 总是允许
}
```

菜单样式:
```
  ◇ Deny   (Enter)
  ○ Allow once
  ○ Always allow
```

选择样式:
```
  ● Deny   (Enter)
  ○ Allow once
  ○ Always allow
```

### 5.2 Thinking 动画

```rust
const SPINNER_FRAMES: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];
```

显示格式: `Thinking... ⠋`

### 5.3 状态栏

格式: `{model}  ↑ {input_tokens}  ↓ {output_tokens}  ~${cost}`

示例: `claude-sonnet-4-6  ↑ 1234  ↓ 5678  ~$0.089`

---

## 测试计划

### T1: TerminalSimulator 测试

```rust
#[test]
fn new_simulator_has_correct_dimensions();
#[test]
fn resize_changes_dimensions();
#[test]
fn send_key_updates_screen();
#[test]
fn ansi_parser_handles_colors();
#[test]
fn ansi_parser_handles_cursor_movement();
```

### T2: 输入系统测试

```rust
#[test]
fn multiline_input_renders_correctly();
#[test]
fn cursor_moves_between_lines();
#[test]
fn backspace_merges_lines();
#[test]
fn history_navigation_works();
#[test]
fn mode_switch_on_slash_or_colon();
```

### T3: 消息渲染测试

```rust
#[test]
fn markdown_paragraph_renders();
#[test]
fn code_block_has_syntax_highlighting();
#[test]
fn bold_text_has_bold_style();
#[test]
fn link_is_underlined();
#[test]
fn tool_call_shows_name_and_status();
```

### T4: 布局测试

```rust
#[test]
fn header_shows_cwd_and_branch();
#[test]
fn status_bar_shows_tokens_and_cost();
#[test]
fn live_region_pins_to_bottom();
#[test]
fn input_box_has_correct_height();
```

### T5: 权限测试

```rust
#[test]
fn permission_menu_renders();
#[test]
fn up_down_navigate_menu();
#[test]
fn enter_selects_option();
#[test]
fn selected_option_has_highlight();
```

---

## 实施顺序

1. **TerminalSimulator** — 基础设施
2. **输入系统** — 核心交互
3. **消息渲染** — 视觉体验
4. **布局架构** — 屏幕组织
5. **权限状态** — 完整闭环

---

## 验收标准

- [ ] 所有 TerminalSimulator 测试通过
- [ ] `cargo test --test tui_tests` 100% 通过
- [ ] 视觉对比 Claude Code 无明显差异
- [ ] 无 clippy warnings
- [ ] 无 unsafe 代码（除 FFI 边界）
