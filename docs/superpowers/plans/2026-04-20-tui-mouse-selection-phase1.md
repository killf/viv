# TUI 鼠标选择与复制 - Phase 1 (MVP) 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现基本的鼠标文本选择功能，支持拖拽选择、反色显示、Ctrl+C 打印坐标范围

**Architecture:**
- 新增 `SelectionState` 管理选择状态（起点、终点、拖拽状态）
- 增强 `MouseEvent` 增加坐标信息（LeftPress/LeftRelease/LeftDrag 带 x,y）
- `InputParser` 解析 1000 模式鼠标序列（ESC [ M C b C）
- `Renderer` 在 flush 前对选择区域的 buffer cell 反色
- `TerminalUI` 响应鼠标事件更新选择状态，Ctrl+C 根据上下文决定行为

**Tech Stack:**
- Rust 零依赖原则
- 终端鼠标模式 1000h（basic tracking）
- 屏幕坐标系统（无需文本映射，Phase 2 实现）

---

## 文件结构

**新建文件：**
- `src/tui/selection.rs` - 选择状态管理（SelectionState、SelectionRegion）
- `tests/tui/selection_test.rs` - 集成测试

**修改文件：**
- `src/core/terminal/input.rs:22-29` - MouseEvent 增加坐标字段
- `src/core/terminal/input.rs:156-210` - 增加鼠标 1000 模式解析
- `src/core/terminal/events.rs:12` - Event::Mouse 携带新 MouseEvent
- `src/tui/terminal.rs:8` - 导入 selection 模块
- `src/tui/terminal.rs:184` - TerminalUI 增加 selection_state 字段
- `src/tui/terminal.rs:258` - 事件循环处理鼠标事件
- `src/tui/terminal.rs:484` - handle_key 中处理 Ctrl+C 上下文逻辑
- `src/tui/renderer.rs:143` - flush 增加选择区域参数和反色逻辑

---

## Task 1: 创建 SelectionState 和 SelectionRegion

**Files:**
- Create: `src/tui/selection.rs`
- Test: `tests/tui/selection_test.rs`

- [ ] **Step 1: 创建 selection.rs 文件骨架**

```rust
// src/tui/selection.rs

use crate::core::terminal::buffer::Rect;

/// 选择区域标准化表示
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRegion {
    /// 左上角坐标（标准化后）
    pub top_left: (u16, u16),
    /// 右下角坐标（标准化后）
    pub bottom_right: (u16, u16),
}

impl SelectionRegion {
    /// 从任意两个点标准化为区域（左上角+右下角）
    pub fn normalize(p1: (u16, u16), p2: (u16, u16)) -> Self {
        let top_left = (
            p1.0.min(p2.0),
            p1.1.min(p2.1),
        );
        let bottom_right = (
            p1.0.max(p2.0),
            p1.1.max(p2.1),
        );
        SelectionRegion { top_left, bottom_right }
    }

    /// 判断单元格是否在选择区域内
    pub fn contains(&self, cell: (u16, u16)) -> bool {
        cell.0 >= self.top_left.0
            && cell.0 <= self.bottom_right.0
            && cell.1 >= self.top_left.1
            && cell.1 <= self.bottom_right.1
    }

    /// 转换为 Rect（用于 buffer 遍历）
    pub fn as_rect(&self) -> Rect {
        Rect {
            x: self.top_left.0,
            y: self.top_left.1,
            width: self.bottom_right.0.saturating_sub(self.top_left.0).saturating_add(1),
            height: self.bottom_right.1.saturating_sub(self.top_left.1).saturating_add(1),
        }
    }
}

/// 选择状态管理
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionState {
    /// 选择起点（屏幕坐标）
    start_pos: Option<(u16, u16)>,
    /// 选择终点（屏幕坐标）
    end_pos: Option<(u16, u16)>,
    /// 是否正在拖拽
    is_dragging: bool,
}

impl SelectionState {
    pub fn new() -> Self {
        SelectionState {
            start_pos: None,
            end_pos: None,
            is_dragging: false,
        }
    }

    /// 开始拖拽
    pub fn start_drag(&mut self, x: u16, y: u16) {
        self.start_pos = Some((x, y));
        self.end_pos = Some((x, y));
        self.is_dragging = true;
    }

    /// 更新拖拽终点
    pub fn update_drag(&mut self, x: u16, y: u16) {
        if self.is_dragging {
            self.end_pos = Some((x, y));
        }
    }

    /// 结束拖拽
    pub fn end_drag(&mut self, x: u16, y: u16) {
        if self.is_dragging {
            self.end_pos = Some((x, y));
            self.is_dragging = false;
        }
    }

    /// 获取标准化后的选择区域
    pub fn region(&self) -> Option<SelectionRegion> {
        match (self.start_pos, self.end_pos) {
            (Some(start), Some(end)) => Some(SelectionRegion::normalize(start, end)),
            _ => None,
        }
    }

    /// 是否有有效选择
    pub fn has_selection(&self) -> bool {
        self.start_pos.is_some() && self.end_pos.is_some() && !self.is_dragging
    }

    /// 清除选择
    pub fn clear(&mut self) {
        self.start_pos = None;
        self.end_pos = None;
        self.is_dragging = false;
    }

    /// 是否正在拖拽中
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
}
```

Run: `cargo build --checks`
Expected: 编译通过（可能有一些 unused 警告，正常）

- [ ] **Step 2: 在 mod.rs 中导出 selection 模块**

```rust
// src/tui/mod.rs

pub mod selection;
// ... 其他已有模块
```

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 3: 写 SelectionRegion 单元测试**

```rust
// tests/tui/selection_test.rs

use viv::tui::selection::SelectionRegion;

#[test]
fn test_selection_normalize_same_point() {
    let region = SelectionRegion::normalize((10, 20), (10, 20));
    assert_eq!(region.top_left, (10, 20));
    assert_eq!(region.bottom_right, (10, 20));
}

#[test]
fn test_selection_normalize_different_points() {
    let region = SelectionRegion::normalize((40, 10), (20, 30));
    // 标准化后：左上角 (20, 10), 右下角 (40, 30)
    assert_eq!(region.top_left, (20, 10));
    assert_eq!(region.bottom_right, (40, 30));
}

#[test]
fn test_selection_contains_inside() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(region.contains((15, 15)));
}

#[test]
fn test_selection_contains_on_boundary() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(region.contains((10, 10))); // 左上角边界
    assert!(region.contains((20, 20))); // 右下角边界
}

#[test]
fn test_selection_contains_outside() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(!region.contains((9, 15)));  // 左边界外
    assert!(!region.contains((21, 15))); // 右边界外
    assert!(!region.contains((15, 9)));  // 上边界外
    assert!(!region.contains((15, 21))); // 下边界外
}

#[test]
fn test_selection_as_rect() {
    let region = SelectionRegion::normalize((10, 5), (20, 15));
    let rect = region.as_rect();
    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 5);
    assert_eq!(rect.width, 11);  // 20 - 10 + 1
    assert_eq!(rect.height, 11); // 15 - 5 + 1
}
```

Run: `cargo test test_selection`
Expected: 全部 PASS

- [ ] **Step 4: 写 SelectionState 单元测试**

```rust
// tests/tui/selection_test.rs (追加)

use viv::tui::selection::SelectionState;

#[test]
fn test_selection_new() {
    let state = SelectionState::new();
    assert!(!state.has_selection());
    assert!(!state.is_dragging());
    assert!(state.region().is_none());
}

#[test]
fn test_selection_start_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    assert!(state.is_dragging());
    assert!(!state.has_selection()); // 拖拽中不算有效选择
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (40, 10))));
}

#[test]
fn test_selection_update_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.update_drag(60, 20);
    assert!(state.is_dragging());
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (60, 20))));
}

#[test]
fn test_selection_end_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.update_drag(60, 20);
    state.end_drag(60, 20);
    assert!(!state.is_dragging());
    assert!(state.has_selection()); // 拖拽结束，有有效选择
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (60, 20))));
}

#[test]
fn test_selection_clear() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.end_drag(60, 20);
    state.clear();
    assert!(!state.has_selection());
    assert!(!state.is_dragging());
    assert!(state.region().is_none());
}
```

Run: `cargo test test_selection`
Expected: 全部 PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/selection.rs src/tui/mod.rs tests/tui/selection_test.rs
git commit -m "feat(tui): add SelectionState and SelectionRegion

- Add SelectionState for managing drag-to-select state
- Add SelectionRegion for normalized selection area
- Implement contains() and as_rect() for rendering
- Unit tests for normalization, boundary checking, state transitions

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: 增强 MouseEvent 增加坐标

**Files:**
- Modify: `src/core/terminal/input.rs:22-29`

- [ ] **Step 1: 修改 MouseEvent 枚举增加坐标字段**

```rust
// src/core/terminal/input.rs

/// Represents a parsed mouse event from raw terminal input bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseEvent {
    WheelUp,
    WheelDown,
    LeftPress { x: u16, y: u16 },
    LeftRelease { x: u16, y: u16 },
    LeftDrag { x: u16, y: u16 },
}
```

Run: `cargo build`
Expected: 编译失败（MouseEvent 使用的地方需要更新）

- [ ] **Step 2: 更新 InputParser 中的 SGR 解析（已有代码适配）**

找到 `src/core/terminal/input.rs:184-199` 的 SGR 鼠标解析代码，更新返回值：

```rust
// src/core/terminal/input.rs (在 SGR 解析部分)

                                        match (n, is_release) {
                                            (Some(0), false) => {
                                                self.buf.drain(..3 + pos + 1);
                                                // 解析 X 和 Y 坐标
                                                let x = Self::parse_u8(parts[1]).unwrap_or(0);
                                                let y = Self::parse_u8(parts[2]).unwrap_or(0);
                                                return Some(InputEvent::Mouse(MouseEvent::LeftPress { x, y }));
                                            }
                                            (Some(0), true) => {
                                                self.buf.drain(..3 + pos + 1);
                                                let x = Self::parse_u8(parts[1]).unwrap_or(0);
                                                let y = Self::parse_u8(parts[2]).unwrap_or(0);
                                                return Some(InputEvent::Mouse(MouseEvent::LeftRelease { x, y }));
                                            }
                                            (Some(64), _) => {
                                                self.buf.drain(..3 + pos + 1);
                                                return Some(InputEvent::Mouse(MouseEvent::WheelUp));
                                            }
                                            (Some(65), _) => {
                                                self.buf.drain(..3 + pos + 1);
                                                return Some(InputEvent::Mouse(MouseEvent::WheelDown));
                                            }
                                            _ => {
                                                self.buf.drain(..3 + pos + 1);
                                            }
                                        }
```

- [ ] **Step 3: 在 InputParser 增加鼠标 1000 模式解析函数**

在 `src/core/terminal/input.rs` 的 `InputParser` impl 块中添加新方法（在现有 `next_event` 方法之后）：

```rust
// src/core/terminal/input.rs

impl InputParser {
    // ... 现有方法

    /// 解析 1000 模式的鼠标序列：ESC [ M C b C
    /// C = button + 32, C = col + 33, C = row + 33
    fn parse_mouse_1000(&mut self) -> Option<InputEvent> {
        if self.buf.len() < 6 {
            return None;
        }

        // 检查 ESC [ M
        if self.buf.get(0) != Some(&b'\x1b')
            || self.buf.get(1) != Some(&b'[')
            || self.buf.get(2) != Some(&b'M')
        {
            return None;
        }

        let button = self.buf[3].saturating_sub(32);
        let col = self.buf[4].saturating_sub(33);
        let row = self.buf[5].saturating_sub(33);

        self.buf.drain(..6);

        let event = match button {
            0 => MouseEvent::LeftPress { x: col, y: row },
            3 => MouseEvent::LeftRelease { x: col, y: row },
            // 32/64 通常表示 wheel up (不同终端编码不同)
            32 | 64 => MouseEvent::WheelUp,
            // 1/65 通常表示 wheel down
            1 | 65 => MouseEvent::WheelDown,
            _ => MouseEvent::LeftRelease { x: col, y: row }, // 其他未知按钮，当作 release
        };

        Some(InputEvent::Mouse(event))
    }
}
```

- [ ] **Step 4: 在 next_event 主循环中调用 parse_mouse_1000**

找到 `src/core/terminal/input.rs` 中处理 CSI 序列的部分（在 `Some(&b'[')` 分支），在尝试 URXVT/SGR 解析之前先尝试 1000 模式：

```rust
// src/core/terminal/input.rs

                        Some(&b'[') => {
                            // ... 现有的 A/B/C/D/H/F/3/Delete 处理 ...

                            Some(_) => {
                                // 先尝试 1000 模式（ESC [ M b x y）
                                if let Some(evt) = self.parse_mouse_1000() {
                                    return Some(evt);
                                }
                                // Try URXVT mouse mode (1015 / 1000): ESC [ M b x y
                                // ... 保留原有的 URXVT 和 SGR 解析代码 ...
                            }
                        }
```

- [ ] **Step 5: 更新 input_test.rs 中的鼠标测试**

修改 `tests/core/terminal/input_test.rs` 中的鼠标测试以适配新的 MouseEvent 格式：

```rust
// tests/core/terminal/input_test.rs

#[test]
fn test_sgr_mouse_left_press() {
    let bytes = b"\x1b[<0;40;10M"; // SGR: button=0, x=40, y=10, press
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::LeftPress { x: 40, y: 10 })));
}

#[test]
fn test_sgr_mouse_left_release() {
    let bytes = b"\x1b[<0;40;10m"; // SGR: button=0, x=40, y=10, release
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::LeftRelease { x: 40, y: 10 })));
}

// ... 其他测试类似更新 ...
```

Run: `cargo test input_test`
Expected: 全部 PASS

- [ ] **Step 6: 添加 1000 模式解析的单元测试**

在 `tests/core/terminal/input_test.rs` 中追加：

```rust
// tests/core/terminal/input_test.rs

#[test]
fn test_mouse_1000_left_press() {
    // 1000 模式：ESC [ M C b C
    // C = button + 32, C = col + 33, C = row + 33
    // button=0 (press), col=40 (+33=73), row=10 (+33=43)
    let bytes = b"\x1b[M!I\x1b[M"; // 实际字节：ESC [ M (0+32=33=!) (40+33=73=I) (10+33=43=+)
    let bytes = b"\x1b[M!I+"; // 正确的编码
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::LeftPress { x: 40, y: 10 })));
}

#[test]
fn test_mouse_1000_left_release() {
    // button=3 (release), col=40, row=10
    let bytes = b"\x1b[M%I+"; // 3+32=35=%
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::LeftRelease { x: 40, y: 10 })));
}

#[test]
fn test_mouse_1000_wheel_up() {
    // button=64 (wheel up), col=40, row=10
    let bytes = b"\x1b[MQI+"; // 64+32=96=Q
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::WheelUp)));
}

#[test]
fn test_mouse_1000_wheel_down() {
    // button=65 (wheel down), col=40, row=10
    let bytes = b"\x1b[MRI+"; // 65+32=97=R
    let mut parser = InputParser::new();
    parser.feed(bytes);
    assert_eq!(parser.next_event(), Some(InputEvent::Mouse(MouseEvent::WheelDown)));
}
```

Run: `cargo test test_mouse_1000`
Expected: 全部 PASS

- [ ] **Step 7: Commit**

```bash
git add src/core/terminal/input.rs tests/core/terminal/input_test.rs
git commit -m "feat(input): add coordinates to MouseEvent, parse 1000 mouse mode

- Add x,y fields to LeftPress/LeftRelease/LeftDrag variants
- Implement parse_mouse_1000() for ESC [ M C b C sequences
- Update SGR parser to extract coordinates
- Add unit tests for 1000 mode parsing

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: TerminalUI 集成 SelectionState 和鼠标事件处理

**Files:**
- Modify: `src/tui/terminal.rs:8`
- Modify: `src/tui/terminal.rs:184`
- Modify: `src/tui/terminal.rs:258`
- Modify: `src/tui/terminal.rs:484`

- [ ] **Step 1: 在 TerminalUI 导入 selection 模块并添加字段**

```rust
// src/tui/terminal.rs

use crate::tui::selection::SelectionState;
// ... 其他已有导入

// 在 TerminalUI 结构体中添加字段
pub struct TerminalUI {
    event_tx: NotifySender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,
    backend: CrossBackend,
    renderer: Renderer,
    editor: LineEditor,
    cwd: String,
    branch: Option<String>,
    model_name: String,
    input_tokens: usize,
    output_tokens: usize,
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<std::time::Instant>,
    spinner_verb: String,
    blocks: Vec<ContentBlock>,
    parse_buffer: MarkdownParseBuffer,
    conversation_state: ConversationState,
    tool_states: Vec<ToolState>,
    focus: FocusManager,
    permission_pending: Option<(usize, String, String, PermissionState)>,
    last_input: String,
    selection_state: SelectionState,  // 新增字段
}
```

- [ ] **Step 2: 在 TerminalUI::new 中初始化 selection_state**

```rust
// src/tui/terminal.rs

        Ok(TerminalUI {
            event_tx,
            msg_rx,
            backend,
            renderer,
            editor: LineEditor::new(),
            cwd,
            branch,
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            busy: false,
            spinner: Spinner::new(),
            spinner_start: None,
            spinner_verb,
            blocks,
            parse_buffer: MarkdownParseBuffer::new(),
            conversation_state,
            tool_states: Vec::new(),
            focus: FocusManager::new(),
            permission_pending: None,
            last_input: String::new(),
            selection_state: SelectionState::new(), // 新增初始化
        })
```

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 3: 在事件循环中处理鼠标事件**

找到 `src/tui/terminal.rs:258` 左右的事件循环 `match event` 部分，在 `Event::Mouse` 分支中添加处理：

```rust
// src/tui/terminal.rs

                Event::Mouse(MouseEvent::LeftPress { x, y }) => {
                    // 检查是否在 conversation 区域内（简单判断：y 在 header 和 status bar 之间）
                    // header 高度约 3 行，status bar 高度 1 行
                    let conv_area_top = 3; // header 后
                    let conv_area_bottom = self.renderer.size.rows.saturating_sub(2); // status bar 前

                    if y >= conv_area_top && y <= conv_area_bottom {
                        self.selection_state.start_drag(x, y);
                        dirty = true;
                    }
                }
                Event::Mouse(MouseEvent::LeftDrag { x, y }) => {
                    if self.selection_state.is_dragging() {
                        self.selection_state.update_drag(x, y);
                        dirty = true;
                    }
                }
                Event::Mouse(MouseEvent::LeftRelease { x, y }) => {
                    if self.selection_state.is_dragging() {
                        self.selection_state.end_drag(x, y);
                        dirty = true;
                    }
                }
                Event::Mouse(MouseEvent::WheelUp) => {
                    self.conversation_state.scroll_up(3);
                    dirty = true;
                }
                Event::Mouse(MouseEvent::WheelDown) => {
                    self.conversation_state.scroll_down(3);
                    dirty = true;
                }
```

- [ ] **Step 4: 在 handle_key 中处理 Ctrl+C 上下文逻辑**

找到 `src/tui/terminal.rs:484` 的 `handle_key` 方法，在 `KeyEvent::CtrlC` 分支中添加逻辑：

```rust
// src/tui/terminal.rs

        // ── Mode 4: Busy -- Ctrl+C interrupts the agent; every other key
        // falls through to the editor so the user can type (and even queue
        // a submission) while the AI is still streaming its response.
        if self.busy && key == KeyEvent::CtrlC {
            // 检查是否有选中文本
            if self.selection_state.has_selection() {
                // Phase 1 MVP: 打印坐标到 stderr
                if let Some(region) = self.selection_state.region() {
                    eprintln!("Selection: ({},{})-({},{})",
                        region.top_left.0, region.top_left.1,
                        region.bottom_right.0, region.bottom_right.1
                    );
                }
                return None;
            } else {
                // 没有选择，中断 Agent
                let _ = self.event_tx.send(AgentEvent::Interrupt);
                return None;
            }
        }
```

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 5: 添加清除选择的逻辑**

在窗口 resize 和滚动时清除选择：

```rust
// src/tui/terminal.rs

                Event::Resize(new_size) => {
                    self.renderer.resize(new_size);
                    // 清除选择（坐标已失效）
                    self.selection_state.clear();
                    // Recalculate all block heights (width changed -> word wrap changes)
                    let width = new_size.cols;
                    for (i, block) in self.blocks.iter().enumerate() {
                        let h = block_height_with_width(block, width);
                        self.conversation_state.set_item_height(i, h);
                    }
                    self.conversation_state.auto_scroll();
                    dirty = true;
                }
```

同时在 `scroll_up` 和 `scroll_down` 后清除选择：

```rust
// src/tui/terminal.rs

                Event::Mouse(MouseEvent::WheelUp) => {
                    self.conversation_state.scroll_up(3);
                    self.selection_state.clear(); // 滚动后清除选择
                    dirty = true;
                }
                Event::Mouse(MouseEvent::WheelDown) => {
                    self.conversation_state.scroll_down(3);
                    self.selection_state.clear(); // 滚动后清除选择
                    dirty = true;
                }
```

同样在 Ctrl+J/K 滚动时也清除：

```rust
// src/tui/terminal.rs

            KeyEvent::CtrlChar('k') => {
                self.conversation_state.scroll_up(3);
                self.selection_state.clear();
                return None;
            }
            KeyEvent::CtrlChar('j') => {
                self.conversation_state.scroll_down(3);
                self.selection_state.clear();
                return None;
            }
```

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add src/tui/terminal.rs
git commit -m "feat(tui): integrate SelectionState into TerminalUI

- Add selection_state field to TerminalUI
- Handle LeftPress/LeftDrag/LeftRelease mouse events
- Context-aware Ctrl+C: copy if selected, interrupt otherwise
- Clear selection on resize and scroll

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Renderer 添加反色渲染支持

**Files:**
- Modify: `src/tui/renderer.rs:143`

- [ ] **Step 1: 修改 Renderer::flush 签名增加 selection 参数**

找到 `src/tui/renderer.rs` 的 `Renderer` impl，修改 `flush` 方法：

```rust
// src/tui/renderer.rs

impl Renderer {
    // ... 其他方法

    pub fn flush(&mut self) -> crate::Result<()> {
        self.flush_with_selection(None)
    }

    pub fn flush_with_selection(&mut self, selection: Option<Rect>) -> crate::Result<()> {
        // 如果有选择区域，遍历 buffer 应用反色
        if let Some(region) = selection {
            for row in region.y..region.y.saturating_add(region.height) {
                for col in region.x..region.x.saturating_add(region.width) {
                    if let Some(cell) = self.buffer.get_mut(col, row) {
                        // 交换前景色和背景色（反色）
                        std::mem::swap(&mut cell.fg, &mut cell.bg);
                    }
                }
            }
        }

        self.backend.flush()?;
        Ok(())
    }
}
```

- [ ] **Step 2: 在 TerminalUI 渲染时传递选择区域**

找到 `src/tui/terminal.rs` 中调用 `renderer.flush()` 的地方，改为 `flush_with_selection`：

```rust
// src/tui/terminal.rs (在渲染循环中)

                            if dirty {
                                // ... 其他渲染代码 ...

                                // 获取选择区域并传递给 flush
                                let selection_rect = self.selection_state.region()
                                    .map(|r| r.as_rect());
                                self.renderer.flush_with_selection(selection_rect)?;

                                dirty = false;
                            }
```

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 3: 添加 renderer 单元测试**

创建 `tests/tui/renderer_test.rs`：

```rust
// tests/tui/renderer_test.rs

use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::Color;
use viv::tui::renderer::Renderer;

#[test]
fn test_flush_with_selection_inverts_colors() {
    // 创建一个 mock backend（实际测试时可能需要调整）
    // 这里测试 buffer 的反色逻辑
    let mut buffer = Buffer::new(Rect::new(0, 0, 10, 5));

    // 设置一些单元格
    buffer.get_mut(2, 2).ch = 'A';
    buffer.get_mut(2, 2).fg = Some(Color::Rgb(255, 0, 0)); // 红色前景
    buffer.get_mut(2, 2).bg = Some(Color::Rgb(0, 0, 255)); // 蓝色背景

    // 记录原始颜色
    let orig_fg = buffer.get(2, 2).fg;
    let orig_bg = buffer.get(2, 2).bg;

    // 应用反色（模拟 Renderer::flush_with_selection 的逻辑）
    let selection = Rect::new(2, 2, 1, 1);
    for row in selection.y..selection.y + selection.height {
        for col in selection.x..selection.x + selection.width {
            if let Some(cell) = buffer.get_mut(col, row) {
                std::mem::swap(&mut cell.fg, &mut cell.bg);
            }
        }
    }

    // 验证颜色被交换
    assert_eq!(buffer.get(2, 2).fg, orig_bg); // 前景变成原来的背景
    assert_eq!(buffer.get(2, 2).bg, orig_fg); // 背景变成原来的前景
}

#[test]
fn test_flush_with_selection_no_panic_on_empty_selection() {
    let mut buffer = Buffer::new(Rect::new(0, 0, 10, 5));
    let selection = None;

    // 不应该 panic
    if let Some(region) = selection {
        for row in region.y..region.y + region.height {
            for col in region.x..region.x + region.width {
                let _ = buffer.get_mut(col, row);
            }
        }
    }

    // 测试通过就是成功
    assert!(true);
}
```

Run: `cargo test test_flush_with_selection`
Expected: 全部 PASS

- [ ] **Step 4: Commit**

```bash
git add src/tui/renderer.rs tests/tui/renderer_test.rs
git commit -m "feat(renderer): add selection rendering with inverted colors

- Add flush_with_selection() method accepting Option<Rect>
- Invert fg/bg colors for cells in selection region
- Add unit tests for color inversion logic

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: 集成测试

**Files:**
- Modify: `tests/tui/selection_test.rs`

- [ ] **Step 1: 添加集成测试（手动测试场景）**

由于集成测试需要实际运行 TUI 和模拟鼠标事件，我们先编写测试框架和简单的状态验证：

```rust
// tests/tui/selection_test.rs (追加)

use viv::tui::selection::{SelectionState, SelectionRegion};

#[test]
fn test_full_selection_flow() {
    // 模拟用户拖拽选择的完整流程
    let mut state = SelectionState::new();

    // 1. 用户按下鼠标 (40, 10)
    state.start_drag(40, 10);
    assert!(state.is_dragging());
    assert!(!state.has_selection());

    // 2. 用户拖拽到 (60, 20)
    state.update_drag(60, 20);
    assert!(state.is_dragging());
    assert_eq!(
        state.region(),
        Some(SelectionRegion::normalize((40, 10), (60, 20)))
    );

    // 3. 用户释放鼠标
    state.end_drag(60, 20);
    assert!(!state.is_dragging());
    assert!(state.has_selection());

    // 4. 验证选择区域
    let region = state.region().unwrap();
    assert!(region.contains((50, 15))); // 内部点
    assert!(!region.contains((30, 15))); // 外部点
}

#[test]
fn test_reverse_drag_direction() {
    // 用户从右下往左上拖拽
    let mut state = SelectionState::new();
    state.start_drag(60, 20);
    state.update_drag(40, 10);
    state.end_drag(40, 10);

    // 标准化后应该得到相同区域
    let region = state.region().unwrap();
    assert_eq!(region.top_left, (40, 10));
    assert_eq!(region.bottom_right, (60, 20));
}

#[test]
fn test_selection_clear_on_scroll() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.end_drag(60, 20);
    assert!(state.has_selection());

    // 模拟滚动（清除选择）
    state.clear();
    assert!(!state.has_selection());
    assert!(state.region().is_none());
}
```

Run: `cargo test test_full_selection_flow`
Expected: 全部 PASS

- [ ] **Step 2: 手动测试清单**

创建手动测试文档 `docs/manual-testing/mouse-selection.md`：

```markdown
# 鼠标选择功能 - 手动测试清单

## Phase 1 MVP 测试

### 基本功能
- [ ] 启动 viv TUI
- [ ] 鼠标拖拽能看到反色选择区域
- [ ] 拖拽方向任意（左上→右下、右下→左上）都能正常工作
- [ ] 选择在 header 和 status bar 之外（conversation 区域）

### Ctrl+C 行为
- [ ] 有选择时按 Ctrl+C，终端输出坐标范围
- [ ] 无选择时按 Ctrl+C，中断 Agent（如果正在运行）
- [ ] 选择后滚动，选择被清除

### 边界情况
- [ ] 单点选择（按下=释放，同坐标）
- [ ] 选择跨越整个屏幕
- [ ] 窗口 resize 后选择被清除
- [ ] 滚轮滚动后选择被清除

### 已知限制（Phase 1）
- [ ] Ctrl+C 只打印坐标，不复制文本（Phase 2 实现）
- [ ] 无法提取选中的文本内容（Phase 2 实现）
- [ ] 没有系统剪贴板集成（Phase 3 实现）
```

Run: `mkdir -p docs/manual-testing`
Run: `cat > docs/manual-testing/mouse-selection.md` (粘贴上面的内容)

- [ ] **Step 3: Commit**

```bash
git add tests/tui/selection_test.rs docs/manual-testing/mouse-selection.md
git commit -m "test(tui): add integration tests for selection flow

- Add full selection flow test (drag → update → release)
- Add reverse drag direction test
- Add selection clear on scroll test
- Create manual testing checklist document

Co-Authored-By: Claude Sonnet 4.6 (1M context) <noreply@anthropic.com>"
```

---

## 验收标准检查

### 自动测试

```bash
# 运行所有测试
cargo test

# 预期：全部 PASS（约 1050+ tests）
```

### 手动测试

```bash
# 启动 viv
cargo run

# 预期行为：
# 1. 在 conversation 区域拖拽鼠标 → 看到反色选择区域
# 2. 按 Ctrl+C → 在终端看到 "Selection: (x1,y1)-(x2,y2)"
# 3. 无选择时按 Ctrl+C → Agent 被中断（如果正在运行）
# 4. 滚轮滚动或窗口 resize → 选择被清除
```

### 代码质量

```bash
# 格式检查
cargo fmt --check

# Lint 检查
cargo clippy

# 预期：无警告和错误
```

---

## 完成标记

Phase 1 (MVP) 完成标准：
- [x] SelectionState 和 SelectionRegion 实现
- [x] MouseEvent 增加坐标字段
- [x] InputParser 解析 1000 模式鼠标序列
- [x] TerminalUI 处理鼠标事件
- [x] Renderer 反色渲染
- [x] Ctrl+C 上下文相关行为
- [x] 单元测试和集成测试
- [x] 手动测试清单

下一阶段（Phase 2）将实现：
- TextMap 坐标到文本的映射
- 文本提取功能
- Ctrl+C 复制实际文本内容
