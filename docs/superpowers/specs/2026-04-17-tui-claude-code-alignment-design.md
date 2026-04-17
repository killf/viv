# TUI — Claude Code 对齐设计

**日期：** 2026-04-17  
**状态：** 待实现  
**方案：** C（分模块并行）

---

## 背景

当前 viv TUI 与 Claude Code 视觉/交互存在多处不一致：

1. 工具权限提示脱离 TUI（直接写 raw stdout，破坏渲染）
2. 无 Header（缺少 cwd、git 分支显示）
3. Footer 过于简陋（仅 `? for shortcuts`）
4. 用户消息前缀 `>` 颜色为 dim，应为 Claude 橙色
5. 输入框无占位文字
6. 不支持多行输入（Shift+Enter 换行）
7. 无 Markdown 渲染（代码块、加粗等）
8. Spinner 在对话区，未独立

---

## 目标布局

```
┌──────────────────────────────────────────────┐
│ ~/project  ⎇ main                            │  header (fixed 1, dim gray)
├──────────────────────────────────────────────┤
│                                              │
│  ● viv  ready                                │  conversation (fill)
│                                              │
│  > 用户消息                                   │
│  ● 助手回复...                                │
│                                              │
├──────────────────────────────────────────────┤
│ ╭────────────────────────────────────────╮   │
│ │ ❯ How can I help you?                  │   │  input (dynamic height, min 3)
│ ╰────────────────────────────────────────╯   │
├──────────────────────────────────────────────┤
│  claude-sonnet-4-6  ↑ 1234 tokens  ~$0.012  │  status bar (fixed 1, dim gray)
└──────────────────────────────────────────────┘
```

---

## 模块设计

### M1 — `tui/header.rs`（新建）

**职责：** 渲染顶部单行状态。

**内容：**
- cwd：`~` 替换 `$HOME`，超过 30 字符时从右截断加 `…`
- git 分支：读 `.git/HEAD`，显示 `⎇ <branch>`；非 git 目录不显示
- 颜色：全 `theme::DIM`

**接口：**
```rust
pub struct HeaderWidget {
    pub cwd: String,       // 已处理好的 cwd 字符串
    pub branch: Option<String>,
}
impl HeaderWidget {
    pub fn from_env() -> Self;  // 读取当前进程的 cwd + .git/HEAD
}
impl Widget for HeaderWidget { ... }
```

**读取 git 分支：** 打开 `.git/HEAD`，解析 `ref: refs/heads/<branch>`，不依赖 git 命令。

---

### M2 — `tui/status.rs`（新建）

**职责：** 渲染底部状态栏，替换现有 footer。

**内容：**
- 模型名：从 `LLMConfig` 传入
- 累计 tokens：由 `AgentContext` 追踪（input + output 分开）
- 估算费用：按固定单价计算（hardcode Sonnet 4.6 价格，可后续扩展）
- 颜色：全 `theme::DIM`
- 格式：`  <model>  ↑ <in_tokens>  ↓ <out_tokens>  ~$<cost>`

**接口：**
```rust
pub struct StatusWidget {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
impl Widget for StatusWidget { ... }
```

**费用计算：** `(input_tokens / 1_000_000 * input_price) + (output_tokens / 1_000_000 * output_price)`，结果格式化为 `$0.000`。价格 hardcode 为常量，后续可从配置读取。

---

### M3 — Input 增强（`tui/input.rs` + `repl.rs`）

#### 3a. 占位文字

`InputWidget` 新增 `placeholder: Option<&str>`：
- 仅在 `content.is_empty()` 时显示
- 颜色：`theme::DIM`

#### 3b. 多行输入

`LineEditor`（在 `repl.rs`）改为多行：

```rust
pub struct LineEditor {
    pub lines: Vec<String>,  // 每行内容
    pub row: usize,          // 光标所在行
    pub col: usize,          // 光标列（byte offset）
}
```

- `Enter` → Submit（将 `lines.join("\n")` 作为输入提交，清空）
- `Shift+Enter` → 在光标处插入新行（`KeyEvent::ShiftEnter`）
- `Up` / `Down` → 在多行间移动（单行时保持原有历史功能暂不做，后续加）
- `InputWidget` 相应支持多行渲染

#### 3c. 动态高度

布局中 input 区域高度 = `min(editor.lines.len() + 2, 8)`（2 = 上下边框，最高 8 行）。

---

### M4 — `tui/permission.rs`（新建）

**职责：** 将工具权限提示集成进对话历史区，消除 raw stdout 写入。

**交互流程：**
1. 需要权限时，在 `history_lines` 末尾追加一条 `PermissionLine`
2. 渲染为：`  ◆ Allow Bash("ls -la")? [y/n] _`（黄色 `◆`，dim 参数）
3. `ask_fn` 改为向 `repl` 发送一个 `AskPermission { tool, input, tx }` 消息，进入 `WaitingPermission` 状态
4. 在 `WaitingPermission` 状态下，`y`/`n` 键响应权限，其他键忽略
5. 权限确认后，`PermissionLine` 更新为 `✓ Allowed` / `✗ Denied`，恢复正常输入

**颜色：**
- `◆`：`theme::SUGGESTION`（蓝紫）
- 工具名：白色
- 参数：`theme::DIM`
- `✓ Allowed`：`theme::SUCCESS`
- `✗ Denied`：`theme::ERROR`

**状态机：**
```
Normal → WaitingPermission → Normal
```

`WaitingPermission` 时输入框 disabled（渲染为 dim，不响应字符输入）。

---

### M5 — `tui/markdown.rs`（新建）

**职责：** 将 Markdown 字符串解析为 `Vec<Line>`，供 `format_assistant_message` 使用。

**支持子集（MVP）：**

| Markdown | 渲染 |
|----------|------|
| `**text**` / `__text__` | bold |
| `` `code` `` | `theme::SUGGESTION` 色 |
| ` ```lang\n...\n``` ` | 代码块：灰色背景边框，内容白色 |
| `# / ## / ###` | 加粗，`###` 后缩进 |
| `- item` / `* item` | `  • item` |
| `1. item` | `  1. item` |
| 纯文本 | 原样 |

**不支持（超出 MVP）：** 表格、图片、嵌套列表、HTML。

**接口：**
```rust
pub fn render_markdown(text: &str) -> Vec<Line>;
```

`format_assistant_message` 改为调用 `render_markdown`，首行加 `● ` 前缀（橙色）。

---

### M6 — `tui/message_style.rs` 更新

| 项目 | 当前 | 目标 |
|------|------|------|
| 用户消息 `>` 颜色 | `theme::DIM` | `theme::CLAUDE`（橙色） |
| 欢迎语 | `● viv  ready` | `● viv  0.1.0  ~/project  ⎇ main` |

欢迎语读取方式：`HeaderWidget::from_env()` 提供 cwd 和 branch，`format_welcome` 接受这两个参数。

---

## 数据流变更

### Token 追踪

在 `agent/context.rs` 的 `AgentContext` 中新增：
```rust
pub input_tokens: u64,
pub output_tokens: u64,
```

`run_agent` 在每次 LLM 响应后更新这两个值（从 SSE 的 `usage` 字段读取）。`repl.rs` 将这两个值传给 `StatusWidget`。

### 权限状态

`repl.rs` 新增状态枚举：
```rust
enum ReplState {
    Normal,
    WaitingPermission { tx: oneshot::Sender<bool> },
}
```

（零依赖：自实现简单的 channel，或用 `std::sync::mpsc`。）

---

## 文件变更清单

| 文件 | 操作 |
|------|------|
| `src/tui/header.rs` | 新建 |
| `src/tui/status.rs` | 新建 |
| `src/tui/permission.rs` | 新建 |
| `src/tui/markdown.rs` | 新建 |
| `src/tui/mod.rs` | 新增 4 个 pub mod |
| `src/tui/input.rs` | 增加 placeholder、多行渲染 |
| `src/tui/message_style.rs` | 用户 `>` 颜色、欢迎语更新 |
| `src/tui/spinner.rs` | 无变更 |
| `src/repl.rs` | 布局更新、LineEditor 多行、状态机、StatusWidget |
| `src/agent/context.rs` | 新增 token 计数字段 |
| `src/agent/run.rs` | 更新 token 计数 |

---

## 测试策略

按 TDD，每个新模块先写测试：

- `tests/tui/header_test.rs` — cwd 截断、git 分支解析
- `tests/tui/status_test.rs` — token 显示格式、费用计算
- `tests/tui/markdown_test.rs` — 各 Markdown 语法的 Line 输出
- `tests/tui/permission_test.rs` — 状态机转换
- `tests/tui/input_test.rs` — 多行编辑、placeholder 渲染（扩展现有测试）

---

## 不在此次范围内

- 历史记录上下翻（Up/Down 浏览历史）
- 语法高亮（代码块内语言着色）
- 鼠标支持
- 水平分屏（多 Agent）
