# TUI Widget 框架设计

> 把单面板对话体验做精：Markdown 渲染、代码高亮、工具调用折叠/展开、虚拟滚动。对标 Claude Code 风格。

## 目标

将 TerminalUI 从扁平的 `Vec<Line>` 文本管线重构为 Widget 化内容区，每个内容块（Markdown 文本、代码块、工具调用）由独立 Widget 管理自己的渲染和状态。

## 非目标

- 分屏布局（后续迭代）
- 完整 CommonMark 解析（只做 Agent 回复常见子集）
- 每语言独立 parser（通用关键词级高亮）
- 行号显示（保持简洁）

---

## 1. Widget 状态管理 + 焦点系统

### 扩展 StatefulWidget

现有 `StatefulWidget` trait 有 `State` 关联类型但未使用。让它成为主力：

```rust
pub trait StatefulWidget {
    type State;
    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}
```

各 Widget 通过 State 管理自己的折叠、滚动、聚焦等状态。

### FocusManager

```rust
struct FocusManager {
    focus_index: usize,       // 当前焦点在哪个可聚焦 widget
    focusable_count: usize,   // 可聚焦 widget 总数（每帧重算）
}
```

- Tab / Shift+Tab 切换焦点
- Enter / Space 触发动作（展开/折叠）
- Esc 回到输入框
- 线性 index 遍历，不做 focus tree — 对话区里只有 ToolCallWidget 需要焦点

### 交互模式

```rust
enum UIMode {
    Normal,  // 键盘输入 → LineEditor（打字）
    Browse,  // Esc 进入 → Tab/方向键导航 → Enter 操作 → Esc 回到 Normal
}
```

Normal 模式下输入框始终接收文字输入。Browse 模式只控制"哪个工具调用被选中"。

---

## 2. Widget 树结构

```
AppWidget
├─ HeaderWidget                    (已有)
├─ ConversationWidget              (新，可滚动容器)
│   ├─ WelcomeWidget               (新，首屏欢迎信息)
│   ├─ UserMessageWidget           (新)
│   ├─ AssistantMessageWidget      (新，包含子 widget)
│   │   ├─ MarkdownBlock           (新，渲染 Markdown 富文本)
│   │   └─ CodeBlock               (新，语法高亮代码块)
│   ├─ ToolCallWidget              (新，可折叠)
│   │   ├─ header: "⚙ Read src/main.rs"    (始终可见)
│   │   └─ body: 输入/输出详情              (折叠/展开)
│   ├─ AssistantMessageWidget      (继续文本...)
│   └─ ...
├─ InputWidget                     (已有，改造)
└─ StatusWidget                    (已有)
```

### 内容模型 — ContentBlock 枚举

Agent 消息解析为结构化数据，驱动 widget 渲染：

```rust
enum ContentBlock {
    Markdown { nodes: Vec<MarkdownNode> },
    CodeBlock { language: Option<String>, code: String },
    ToolCall {
        id: usize,
        name: String,
        input: String,
        output: Option<String>,
        error: Option<String>,
    },
    UserMessage { text: String },
}
```

`TerminalUI` 维护 `Vec<ContentBlock>` 作为对话内容，每帧根据它构建 widget 树渲染。

### 高度预计算

每个 ContentBlock 根据给定宽度算出渲染高度（word-wrap 后的行数）。ConversationWidget 用缓存的高度做滚动计算，不渲染不可见区域。

---

## 3. Markdown 渲染

轻量行级 Markdown parser，只支持 Agent 回复常见子集。

### 支持的语法

块级元素：

| 语法 | 渲染 |
|------|------|
| `# 标题` (1-6 级) | 粗体 + 颜色区分 |
| `- 列表` / `1. 有序列表` | 缩进 + bullet/数字 |
| `> 引用块` | 左边栏 `│` + dim 颜色 |
| ` ```lang ` 代码块 | 交给 CodeBlock 处理 |
| `---` 分隔线 | 水平线 |

行内元素：

| 语法 | 渲染 |
|------|------|
| `**粗体**` | bold |
| `*斜体*` | dim（终端无真正斜体） |
| `` `行内代码` `` | Claude 橙色 |
| `[链接](url)` | 蓝色下划线，只显示文本 |

### 数据模型

```rust
enum MarkdownNode {
    Heading { level: u8, text: Vec<InlineSpan> },
    Paragraph { spans: Vec<InlineSpan> },
    List { ordered: bool, items: Vec<Vec<InlineSpan>> },
    Quote { spans: Vec<InlineSpan> },
    CodeBlock { language: Option<String>, code: String },
    HorizontalRule,
}

enum InlineSpan {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { text: String, url: String },
}
```

### 渲染样式（Claude Code 风格）

```
# 标题      → 白色粗体，上下各留 1 空行
- 列表      → "  • " 前缀，嵌套每级缩进 2 格
> 引用      → 左侧 "│ " dim 灰色竖线，文字 dim
`行内代码`  → Claude 橙色  Rgb(215, 119, 87)
**粗体**    → bold
*斜体*      → dim
链接        → 蓝色下划线，文本可见，url 不显示
```

### 流式解析

Markdown 解析器维护一个 buffer，每次 TextChunk 到达时增量解析：

- 完整行 → 立即解析为 MarkdownNode
- 未闭合的代码块 → 持续 buffer 直到 ``` 闭合
- 未完成的行 → 保留在 buffer 等下一块

---

## 4. 代码块语法高亮

基于正则的通用 tokenizer，一套规则覆盖大多数 C 系语言。

### Token 类型

```rust
enum TokenKind {
    Keyword,      // if, for, fn, def, class, return...
    String,       // "..." '...' `...` """..."""
    Comment,      // // ... # ... /* ... */
    Number,       // 42, 3.14, 0xFF, 1_000
    Type,         // 大写开头的标识符: String, Vec, None
    Function,     // 标识符后紧跟 ( : foo(
    Operator,     // = + - * / < > ! & | ^ % :: -> =>
    Punctuation,  // { } ( ) [ ] ; , .
    Plain,        // 其他
}
```

### 合并关键词表

```
通用:   if else for while return break continue match switch case
Rust:   fn let mut pub struct enum impl trait use mod async await self Self
Python: def class import from as with try except raise lambda yield
JS/TS:  function const var export default import async await typeof
Go:     func package range defer go chan select
Shell:  then fi do done elif esac
```

所有关键词放一个 HashSet，不区分语言。language hint 暂不影响高亮规则。

### 着色方案

```
Keyword     → 蓝色粗体       Rgb(110, 150, 255)
String      → 绿色           Rgb(120, 200, 120)
Comment     → dim 灰色       Rgb(100, 100, 100)
```

注意：`#` 注释仅在行首且后跟空格时识别，避免误标 Rust `#[attr]`、CSS `#id` 等。
Number      → 橙色           Rgb(215, 160, 87)
Type        → 青色           Rgb(100, 200, 200)
Function    → 黄色           Rgb(230, 220, 110)
Operator    → 白色           Rgb(200, 200, 200)
Punctuation → dim            Rgb(150, 150, 150)
Plain       → 默认白色
```

### 代码块渲染

```
╭─ rust ──────────────────────────────────────────╮
│ fn main() {                                      │
│     println!("hello");                           │
│ }                                                │
╰──────────────────────────────────────────────────╯
```

用 Block widget (Rounded border) 包裹，左上角显示语言标签。

### Tokenizer 实现

逐字符状态机：

```
Normal → " 进入 String → 闭合 " 回到 Normal
Normal → // 进入 LineComment → 行尾回到 Normal
Normal → # 且为行首+后跟空格 → LineComment（Python/Shell 注释，避免误标 Rust #[attr]）
Normal → /* 进入 BlockComment → */ 回到 Normal
Normal → 数字开头进入 Number → 非数字/点/x 回到 Normal
Normal → 字母开头收集标识符 → 查关键词表分类
```

---

## 5. 工具调用展示

### 折叠态（默认）— 单行摘要

```
 ⚙ Read  src/main.rs                              ✓ 35 lines
 ⚙ Bash  cargo test                                ✓ 0.8s
 ⚙ Edit  src/lib.rs                                ✗ error: not unique
 ⚙ Grep  pattern: "fn main"                        ⚙ running...
```

结构：`图标 + 工具名(粗体) + 输入摘要(dim) + 右对齐状态`

状态指示：

- `✓` 绿色 — 成功，附带简要结果（行数/耗时/匹配数）
- `✗` 红色 — 失败，附带错误摘要
- `⚙` 动画 — 运行中

### 输入摘要提取规则

```
Read/Write/Edit  → file_path
Bash             → command (截断到 60 字符)
Grep             → pattern
Glob             → pattern
WebFetch         → url
SubAgent         → description 前 40 字符
其他             → input JSON 第一个字段值
```

### 展开态

Browse 模式下 Enter 切换：

```
 ⚙ Read  src/main.rs                              ✓ 35 lines
 ╭─ input ─────────────────────────────────────────────────────╮
 │ {"file_path": "/data/dlab/viv/src/main.rs"}                │
 ╰─────────────────────────────────────────────────────────────╯
 ╭─ output ────────────────────────────────────────────────────╮
 │ 1  use viv::{Result, ...};                                  │
 │ 2  fn main() -> Result<()> {                                │
 │ ...                                                         │
 │ 35 }                                                        │
 ╰─────────────────────────────────────────────────────────────╯
```

- input 和 output 各用 Block 包裹
- output 如果是代码，走 CodeBlock 高亮渲染
- output 超过 20 行截断，底部显示 `... (N more lines)`
- 展开态下 j/k 可滚动 output

### ToolCallWidget 状态

```rust
struct ToolCallState {
    folded: bool,           // 折叠/展开
    status: ToolStatus,     // Running / Success / Error
    output_scroll: u16,     // 展开时 output 滚动偏移
}

enum ToolStatus {
    Running,
    Success { summary: String },
    Error { message: String },
}
```

### 聚焦视觉反馈

Browse 模式下选中的 ToolCallWidget 左侧加亮色竖线：

```
 ┃ ⚙ Read  src/main.rs                            ✓ 35 lines    ← 选中
   ⚙ Bash  cargo test                              ✓ 0.8s
```

---

## 6. 滚动与导航

### 虚拟滚动

```rust
struct ConversationState {
    scroll_offset: u16,      // 视口顶部在虚拟空间中的位置
    viewport_height: u16,    // 可见区域高度
    auto_follow: bool,       // 是否自动跟随新内容
    item_heights: Vec<u16>,  // 每个 block 的缓存高度
    total_height: u16,       // 所有 block 高度之和
}
```

### 高度计算时机

- 新 ContentBlock 加入 → 算一次高度，append 到 item_heights
- 终端 Resize → 全部重算（宽度变，word-wrap 行数变）
- 流式 TextChunk 到达 → 只更新最后一个 block 的高度

### 滚动规则

Normal 模式（打字时）：

- 新内容到达 → 自动滚动到底部（auto-follow）
- 用户手动滚动 → 取消 auto-follow，右下角显示 `↓ new` 提示
- End 或 Ctrl+E → 恢复 auto-follow

Browse 模式（Esc 进入）：

```
↑/k  ↓/j         → 逐行滚动
PageUp / PageDown → 翻页（viewport_height - 2 行）
Home / g          → 跳到顶部
End / G           → 跳到底部
Tab / Shift+Tab   → 跳到下一个/上一个 ToolCallWidget
Enter             → 展开/折叠当前选中的 ToolCallWidget
Esc               → 回到 Normal 模式
```

### 渲染优化

遍历 items，累加 height：

1. 跳过 scroll_offset 之前的 block
2. 第一个可见 block — 可能只渲染下半部分（被顶部裁剪）
3. 中间完全可见的 blocks — 完整渲染
4. 最后一个可见 block — 可能只渲染上半部分（被底部裁剪）
5. 超出 viewport 后停止

不可见的 block 完全不调用 render。

### 滚动条

右侧 1 列宽的滚动指示器，只在内容超出视口时显示：

```
┃   ← 当前视口范围
┃
│
│
│
```

`┃` 表示视口位置，`│` 表示其他区域。

---

## 7. 整体架构变更

### TerminalUI 新结构

```rust
struct TerminalUI {
    // 通信（不变）
    event_tx: NotifySender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,

    // 渲染（不变）
    backend: CrossBackend,
    renderer: Renderer,

    // 内容模型（新）
    blocks: Vec<ContentBlock>,
    parse_buffer: MarkdownParseBuffer,

    // Widget 状态（新）
    conversation_state: ConversationState,
    tool_states: Vec<ToolCallState>,
    focus: FocusManager,

    // 交互模式（新）
    mode: UIMode,

    // 输入（已有）
    editor: LineEditor,

    // 已有
    spinner: Spinner,
    busy: bool,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
}
```

### 消息处理流程

```
TextChunk(s) → parse_buffer.push(s) → 产出 MarkdownNode → 更新 blocks 末尾
              （若产出 MarkdownNode::CodeBlock → 提升为独立的 ContentBlock::CodeBlock）
ToolStart    → blocks.push(ContentBlock::ToolCall { id: tool_seq++, status: Running })
ToolEnd      → 从 blocks 末尾向前查找第一个 id 匹配且 status=Running 的 ToolCall → 更新 status + output
```

ToolCall 通过递增序号 `tool_seq` 匹配 ToolStart/ToolEnd（Agent 消息协议保证顺序对应）。

### 每帧渲染流程

```
1. drain msg_rx → 更新 blocks + tool_states
2. poll events → 更新 mode / focus / scroll / editor
3. Layout::split(全屏) → [header, conversation, input, status]
4. HeaderWidget.render(header_area)
5. ConversationWidget.render(conv_area, blocks, tool_states, focus, conversation_state)
6. InputWidget.render(input_area)
7. StatusWidget.render(status_area)
8. Renderer::flush(diff → backend)
```

### 新增文件

```
src/tui/
├── conversation.rs     // ConversationWidget — 虚拟滚动容器
├── markdown.rs         // MarkdownBlock widget + 解析器
├── code_block.rs       // CodeBlock widget
├── syntax.rs           // 通用语法高亮 tokenizer
├── tool_call.rs        // ToolCallWidget
├── focus.rs            // FocusManager
└── content.rs          // ContentBlock 枚举 + 解析逻辑
```

新增 7 个文件，改造 `src/bus/terminal.rs`，现有文件基本不动。
