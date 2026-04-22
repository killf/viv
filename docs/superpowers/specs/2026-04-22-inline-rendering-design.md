# Inline 渲染（半全屏）设计

**日期：** 2026-04-22
**状态：** 待实现
**目标：** 从 alt-screen 全屏 TUI 迁移到 Claude Code 式 inline 渲染，保留终端 scrollback

---

## 背景

当前 `TerminalUI::new` 在启动时调用 `backend.enter_alt_screen()`，占用整个终端屏幕；退出后 alt-screen 被释放、对话内容不可回看。用户希望：

- 启动后不清屏、不进 alt-screen
- 对话内容打进终端原生 scrollback，永久可回看
- 底部固定区域用于实时交互（输入、spinner、streaming 中的助手回复）
- 滚动、鼠标选择、右键菜单完全交给终端原生能力

此设计取代 2026-04-20 未落地的 inline-rendering 草案；同时对齐记忆档案 `project_inline_rendering.md` 的两层模型。

## 架构 — 两层存储

| 层 | 存储位置 | 绘制责任 | 可变性 |
|---|---|---|---|
| Committed scrollback | 终端 native scrollback | 终端（viv 只 `write_all`） | 只追加 |
| Live region | `live_blocks: Vec<LiveBlock>` + `editor` + `status` | `Renderer` 每帧重绘 | 每帧全重绘 |

**数据流：**

```
AgentMessage
  ↓
handle_agent_message()
  ├─ UserMessage          → 直接 commit（输入已提交）
  ├─ Status / Error       → 直接 commit
  ├─ TextChunk            → append 到 live 的 in-flight Markdown
  ├─ ToolStart            → push live ToolCall(Running)
  ├─ ToolEnd / ToolError  → mark 对应 ToolCall 为 Committing
  ├─ PermissionRequest    → push live PermissionPrompt
  ├─ PermissionResponse   → 替换为结果行并 commit
  └─ Done                 → live Markdown 尾段封顶 → commit
  ↓
dirty = true
  ↓
主循环 render_frame()
  1. 取所有 state == Committing 的 live_blocks
  2. cursor_up(last_live_rows) + \x1b[0J 清到屏末
  3. 按插入顺序把 committing block 写成文本 + \n（进 scrollback）
  4. 从 live_blocks 移除已 committed
  5. 绘制 live region（live_blocks + 空行 + input + status）
  6. 光标定位到 input 内
  7. 记录 last_live_rows
```

**删除的状态：** `ConversationState`、`SelectionState`、`FocusManager`、`TextMap`、`welcome_anim`。

## Live region 布局

```
┌─ (scrollback，终端管) ─────────────────────────┐
│  ...                                            │
│  > 用户问题一                                   │   committed
│  ● 助手已完成的 Markdown 段落                    │   committed
│  ⏺ Bash(ls) ✓                                  │   committed
├─ live region（每帧重绘） ───────────────────────┤
│  ● 助手 in-flight Markdown 段落                  │   live block
│  ⏺ Read(foo.rs) … 运行中                       │   live block
│                                                 │   空行
│  ╭─────────────────────────────────────╮      │
│  │ ❯                                    │      │   input box (3-8 行)
│  ╰─────────────────────────────────────╯      │
│    ✶ crafting… · claude-sonnet-4-6 · ↑ 1.2k    │   status (1 行)
└─────────────────────────────────────────────────┘
```

**关键调整：**
- spinner 融进 status line（不再单独一行）
- 顶部 header (cwd/branch) 删除，cwd/branch 仅在 welcome 中出现一次
- welcome 启动时静态打印一次到 scrollback，无 fade-in 动画
- 权限菜单出现时替换输入框为 `PermissionWidget`，结束后恢复，结果行 commit 进 scrollback

**高度：** `live_rows = Σ(live_blocks.rows) + 1(空行) + editor_height(3-8) + 1(status)`，`live_blocks.rows` 复用现有 `block_height_with_width`。

## Commit 状态机

```rust
enum BlockState { Live, Committing }
```

`Committing` 仅存在于"本帧待写入 scrollback"的一瞬；写完后 block 直接从 `live_blocks` 移除，不保留 `Committed` 态。

| Block 类型 | Live → Committing 触发点 |
|---|---|
| `Markdown`（in-flight） | `MarkdownParseBuffer::push` 吐出封闭 `MarkdownNode`（段落/代码块/列表） |
| `Markdown`（尾部） | `AgentMessage::Done` 时 flush 剩余 |
| `ToolCall` | `AgentMessage::ToolEnd` / `ToolError` |
| `PermissionPrompt` | 用户 Enter 选定选项后（先替换为结果行） |
| `UserMessage` | Enter 提交即 commit（无 live 阶段） |
| `Status` / `Error` | 产生即 commit |

**每帧顺序（严格）：** 先 commit 顶部，再重绘剩余 live。committed 之后不可回读。

## 移除清单

**整块删除：**
- `tui/focus.rs` — 无内部 focus；committed 不可交互
- `tui/selection.rs` — 无内部鼠标选择
- `tui/text_map.rs` — 仅服务于选择复制
- `tui/conversation.rs` — 无 scroll / viewport / scrollbar

**`tui/terminal.rs` 内部移除：**
- `backend.enter_alt_screen()` / `leave_alt_screen()` 调用
- `KeyEvent::CtrlChar('k')` / `CtrlChar('j')` 分支
- `Event::Mouse(_)` 全部分支
- `WelcomeAnimState`、welcome fade-in 动画代码
- `ConversationState::auto_scroll()` / `visible_items()` / `render_scrollbar()` 调用
- 字段：`blocks`（被 `live_blocks` 替代）、`welcome_anim`、`selection_state`、`focus`

**`core/terminal/events.rs`：** 关闭鼠标跟踪（不发 `1006/1015h`），滚轮回归 shell。鼠标事件解析层可保留但不订阅。

**`core/terminal/backend.rs`：** `enter_alt_screen` / `leave_alt_screen` 保留 trait 方法（未来可能用），`TerminalUI::new` 不再调用。

**保留：**
- `Renderer` + `Buffer` + `Paragraph` 换行：用于绘制 live region 的小 buffer（6-15 行量级）
- `MarkdownParseBuffer`：streaming 解析逻辑不变
- `MarkdownBlockWidget` / `CodeBlockWidget` / `ToolCallWidget`：既服务 live 重绘，也服务 committing 时的一次性文本生成
- `WelcomeWidget`：启动静态渲染一次到 stdout
- `StatusWidget`：扩展为包含 spinner verb

## Resize & 边界情况

**Resize（SIGWINCH → `Event::Resize`）：**
1. 不触碰 scrollback（终端已 reflow）
2. 重算所有 live_blocks 高度（新 width）
3. 下一帧：`cursor_up(last_live_rows)` + `\x1b[0J` + 重绘

**关键不变量：** 维护 `last_live_rows` = 上一帧实际写了几行；每帧只清这么多，永远不清越界。

**终端太窄（width < 40）：** status line 退化为仅 token 计数。

**Live region 比终端高：** in-flight Markdown block 若 > `terminal_height - 10`，只显示最后 N 行，附加 `⋯ 12 more lines`；commit 时写入完整内容。

**Windows：** `\x1b[nA` / `\x1b[0J` 在启用 VT mode 的 Windows 10+ conhost / Windows Terminal 上均可用；不做 fallback。

## 启动 / 退出

**启动（`TerminalUI::new`）：**
1. `enable_raw_mode()`
2. DECSCUSR steady bar cursor (`\x1b[6 q`)
3. 不调用 `enter_alt_screen`
4. `last_live_rows = 0`
5. 首次收到 `AgentMessage::Ready { model }` 时，把 `WelcomeWidget` 静态渲染一次写入 stdout（进 scrollback），之后不再重绘

**退出（`cleanup`）：**
1. `cursor_up(last_live_rows)` + `\x1b[0J`
2. `write "Bye!\n"`（进 scrollback）
3. cursor style reset `\x1b[0 q`
4. `disable_raw_mode()`

用户退出后终端保留完整对话历史，可原生滚动/复制。

## 测试策略

- **单元测试** `tests/tui/live_region.rs`：给定 `live_blocks + editor + status`，断言 `frame()` 输出的字节序列包含预期的 `ESC[nA`、`ESC[0J`、committed 文本、live 文本、光标定位 `ESC[y;xH`。
- **集成测试** `tests/tui/inline_flow.rs`：mock `Backend`（写入 `Vec<u8>`），跑 scripted `AgentMessage` 序列（Ready → TextChunk ×N → ToolStart → ToolEnd → Done），断言最终 stdout 字节流。
- **快照** `tests/tui/snapshots/`：把典型对话的 stdout 字节序列落快照，后续重构回归对比。

不做 e2e（真实终端输出难以稳定断言；字节流快照已覆盖回归需求）。

## 非目标

- 不做内部 scroll / viewport / scrollbar
- 不做鼠标交互（选择、点击、滚轮）
- 不做工具块交互式展开/折叠（commit 后不可交互）
- 不做 welcome fade-in 动画
- 不做 alt-screen fallback
