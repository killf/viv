# TUI 鼠标选择与复制功能设计文档

**日期**: 2026-04-20
**状态**: 设计阶段
**作者**: Claude Code

---

## 1. 概述

为 viv 的 TUI 界面添加完整的鼠标文本选择和复制功能，支持字符级精确选择、跨消息块选择、反色显示，并通过 FFI 调用系统剪贴板 API。

### 1.1 设计目标

- **字符级精确选择**: 鼠标拖拽可以精确到单个字符
- **跨块选择**: 可以连续选中多条消息中的任意字符
- **反色显示**: 选中文本以前景色和背景色互换方式高亮
- **智能快捷键**: Ctrl+C 根据上下文自动切换（有选择=复制，无选择=中断 Agent）
- **系统剪贴板**: 通过 FFI 调用原生 API 复制到操作系统剪贴板

### 1.2 非目标

- 不支持矩形块选择（列模式）
- 不支持多选区（只能有一个连续选择区）
- 不依赖外部命令（如 `xclip`、`wl-copy`），纯 FFI 实现

---

## 2. 核心架构

### 2.1 新增组件

```
src/tui/
├── selection.rs           # 选择状态管理
├── clipboard/             # 剪贴板后端（新增模块）
│   ├── mod.rs
│   ├── x11.rs
│   ├── wayland.rs
│   ├── macos.rs
│   └── windows.rs
└── text_map.rs            # 坐标到文本的映射（可选）
```

### 2.2 核心数据结构

#### SelectionState（选择状态）

```rust
pub struct SelectionState {
    /// 选择起点（屏幕坐标）
    start_pos: Option<(u16, u16)>,
    /// 选择终点（屏幕坐标）
    end_pos: Option<(u16, u16)>,
    /// 是否正在拖拽
    is_dragging: bool,
}

impl SelectionState {
    /// 开始拖拽
    fn start_drag(&mut self, x: u16, y: u16);

    /// 更新拖拽终点
    fn update_drag(&mut self, x: u16, y: u16);

    /// 结束拖拽
    fn end_drag(&mut self, x: u16, y: u16);

    /// 获取标准化后的选择区域
    fn region(&self) -> Option<Rect>;

    /// 是否有有效选择
    fn has_selection(&self) -> bool;

    /// 清除选择
    fn clear(&mut self);
}
```

#### SelectionRegion（选择区域计算）

```rust
pub struct SelectionRegion {
    /// 标准化后的左上角坐标
    top_left: (u16, u16),
    /// 标准化后的右下角坐标
    bottom_right: (u16, u16),
}

impl SelectionRegion {
    /// 从任意两个点标准化为区域
    fn normalize(p1: (u16, u16), p2: (u16, u16)) -> Self;

    /// 判断单元格是否在选择区域内
    fn contains(&self, cell: (u16, u16)) -> bool;

    /// 转换为 Rect
    fn as_rect(&self) -> Rect;
}
```

#### ClipboardBackend（剪贴板后端）

```rust
pub enum ClipboardBackend {
    X11,
    Wayland,
    MacOS,
    Windows,
}

impl ClipboardBackend {
    /// 检测当前平台可用的剪贴板后端
    fn detect() -> Option<Self>;

    /// 复制文本到系统剪贴板
    fn copy(text: &str) -> Result<()>;
}
```

### 2.3 增强的鼠标事件

```rust
pub enum MouseEvent {
    WheelUp,
    WheelDown,
    LeftPress { x: u16, y: u16 },      // 新增：带坐标
    LeftRelease { x: u16, y: u16 },    // 新增：带坐标
    LeftDrag { x: u16, y: u16 },       // 新增：拖拽
}
```

---

## 3. 交互流程

### 3.1 完整数据流

```
用户操作
  ↓
InputParser 解析终端字节流
  ↓
MouseEvent::LeftPress / LeftDrag / LeftRelease
  ↓
TerminalUI.handle_mouse_event()
  ↓
SelectionState.start_drag() / update_drag() / end_drag()
  ↓
dirty = true → 触发渲染循环
  ↓
Renderer.render(blocks, selection_state.region())
  ↓
Widget::render() → Buffer + TextMap 构建
  ↓
Renderer.flush_with_selection()
  ├─ 遍历 buffer，对选择区域的 cell 反色
  └─ backend.write() 输出到终端
  ↓
用户按 Ctrl+C
  ↓
TerminalUI.handle_key(KeyEvent::CtrlC)
  ├─ if selection_state.has_selection()
  │   └─ 提取文本 → ClipboardBackend::copy()
  └─ else
      └─ event_tx.send(AgentEvent::Interrupt)
```

### 3.2 鼠标事件处理

```rust
// TerminalUI 中的事件循环
match event {
    Event::Mouse(MouseEvent::LeftPress { x, y }) => {
        if in_conversation_area(x, y) {
            self.selection_state.start_drag(x, y);
            dirty = true;
        }
    }

    Event::Mouse(MouseEvent::LeftDrag { x, y }) => {
        if self.selection_state.is_dragging {
            self.selection_state.update_drag(x, y);
            dirty = true;
        }
    }

    Event::Mouse(MouseEvent::LeftRelease { x, y }) => {
        if self.selection_state.is_dragging {
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

    Event::Mouse(_) => {}
}
```

### 3.3 Ctrl+C 上下文相关处理

```rust
KeyEvent::CtrlC => {
    if self.selection_state.has_selection() {
        // 复制选中文本
        let text = self.extract_selection_text()?;
        ClipboardBackend::copy(&text)?;
        self.status_bar.show("已复制到剪贴板");
    } else {
        // 中断 Agent
        let _ = self.event_tx.send(AgentEvent::Interrupt);
    }
}
```

---

## 4. 渲染与反色

### 4.1 反色处理位置

在 `Renderer::flush()` 前统一处理，避免修改所有 Widget 签名：

```rust
impl Renderer {
    fn flush_with_selection(&mut self, selection: &Option<Rect>) -> Result<()> {
        // 正常渲染完成...

        // 如果有选择区域，遍历 buffer 应用反色
        if let Some(region) = selection {
            for row in region.y..region.y + region.height {
                for col in region.x..region.x + region.width {
                    if let Some(cell) = self.buffer.get_mut(col, row) {
                        // 交换前景色和背景色
                        std::mem::swap(&mut cell.fg, &mut cell.bg);
                    }
                }
            }
        }

        self.backend.flush()
    }
}
```

### 4.2 CJK 宽字符处理

由于 `char_width()` 已经处理了宽字符占两列的问题，反色遍历时只需确保遍历到实际占用的所有单元格即可。CJK 字符的左半列和右半列都会被标记为同一个 `CellSource`，复制时会完整复制该字符。

---

## 5. 文本提取

### 5.1 坐标映射表

在渲染时构建屏幕坐标到文本内容的映射：

```rust
struct CellSource {
    block: usize,      // blocks 中的索引
    span: usize,       // 该 block 中 span 的索引
    byte_offset: usize // span 中的字节偏移量
}

struct TextMap {
    // 稀疏存储：只为有文本的单元格记录
    cells: HashMap<(u16, u16), CellSource>,
}

impl TextMap {
    fn set_source(&mut self, x: u16, y: u16, source: CellSource) {
        self.cells.insert((x, y), source);
    }

    fn get_source(&self, x: u16, y: u16) -> Option<&CellSource> {
        self.cells.get(&(x, y))
    }
}
```

### 5.2 Widget 渲染时构建映射

```rust
impl Widget for ParagraphWidget {
    fn render(&self, area: Rect, buf: &mut Buffer, map: &mut TextMap) {
        let mut byte_offset = 0;
        for (row, line) in wrapped_lines.iter().enumerate() {
            let mut col = 0;
            for ch in line.chars() {
                let screen_y = area.y + row as u16;
                let screen_x = area.x + col;

                // 记录映射关系
                map.set_source(
                    screen_x,
                    screen_y,
                    CellSource {
                        block: self.block_index,
                        span: self.span_index,
                        byte_offset,
                    },
                );

                // 渲染字符
                buf.get_mut(screen_x, screen_y).ch = ch;
                col += char_width(ch);
                byte_offset += ch.len_utf8();
            }
        }
    }
}
```

### 5.3 提取选中文本

```rust
impl TerminalUI {
    fn extract_selection_text(&self) -> Result<String> {
        let region = self.selection_state.region()
            .ok_or(Error::NoSelection)?;

        let mut selected_blocks: HashSet<usize> = HashSet::new();
        let mut selected_spans: HashMap<(usize, usize), Vec<Range<usize>>> = HashMap::new();

        // 遍历选择区域，收集所有选中的 (block, span, byte_range)
        for row in region.top..region.bottom {
            for col in region.left..region.right {
                if let Some(source) = self.text_map.get_source(col, row) {
                    selected_blocks.insert(source.block);
                    selected_spans
                        .entry((source.block, source.span))
                        .or_insert_with(Vec::new)
                        .push(source.byte_offset..source.byte_offset + 1);
                }
            }
        }

        // 合并连续范围并提取文本
        let mut result = String::new();
        for block_idx in selected_blocks.into_iter().sorted() {
            let block = &self.blocks[block_idx];
            // 根据选中的 spans 提取文本...
            // 处理 line wrap、CJK 字符边界等
        }

        Ok(result)
    }
}
```

---

## 6. 剪贴板集成（FFI）

### 6.1 平台检测

```rust
impl ClipboardBackend {
    fn detect() -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            // 优先检测 Wayland（更现代）
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                return Some(ClipboardBackend::Wayland);
            }
            // 降级到 X11
            if std::env::var("DISPLAY").is_ok() {
                return Some(ClipboardBackend::X11);
            }
        }
        #[cfg(target_os = "macos")]
        {
            return Some(ClipboardBackend::MacOS);
        }
        #[cfg(target_os = "windows")]
        {
            return Some(ClipboardBackend::Windows);
        }
        None
    }
}
```

### 6.2 Linux X11 FFI

```rust
// src/tui/clipboard/x11.rs

use std::os::raw::{c_char, c_int, c_ulong};
use std::ffi::CString;

#[repr(C)]
struct Display {
    _private: [u8; 0],
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(_: *const c_char) -> *mut Display;
    fn XCloseDisplay(dpy: *mut Display) -> c_int;
    fn XSetSelectionOwner(
        dpy: *mut Display,
        selection: c_ulong,
        owner: c_ulong,
        time: c_ulong,
    ) -> c_int;
    fn XChangeProperty(
        dpy: *mut Display,
        window: c_ulong,
        property: c_ulong,
        typ: c_ulong,
        format: c_int,
        mode: c_int,
        data: *const u8,
        nelements: c_int,
    ) -> c_int;
}

pub fn copy(text: &str) -> Result<()> {
    // FFI 调用实现...
    Ok(())
}
```

### 6.3 错误处理

- 启动时检测剪贴板支持，不支持时警告但继续运行
- 复制失败时在状态栏显示错误：`无法复制到剪贴板：{原因}`
- FFI 调用失败不崩溃，返回 `Result`

---

## 7. 鼠标模式启用

### 7.1 终端模式配置

```rust
// src/core/terminal/backend.rs

fn enter_alt_screen(&mut self) -> Result<()> {
    if !self.in_alt_screen {
        self.stdout.write_all(ENTER_ALT_SCREEN)?;
        // 启用鼠标模式：1000h (basic tracking)
        // 不启用 1006h (SGR)，避免破坏我们自己的事件解析
        self.stdout.write_all(ENABLE_MOUSE_1000)?;
        self.stdout.flush()?;
        self.in_alt_screen = true;
    }
    Ok(())
}

fn leave_alt_screen(&mut self) -> Result<()> {
    if self.in_alt_screen {
        self.stdout.write_all(DISABLE_MOUSE_1000)?;
        self.stdout.write_all(LEAVE_ALT_SCREEN)?;
        self.stdout.flush()?;
        self.in_alt_screen = false;
    }
    Ok(())
}
```

### 7.2 InputParser 增强解析

```rust
// 解析 1000 模式的鼠标序列：ESC [ M C b C
// C = button + 32, C = col + 33, C = row + 33
fn parse_mouse_1000(&mut self) -> Option<InputEvent> {
    if self.buf.len() < 6 {
        return None;
    }

    // 检查 ESC [ M
    if &self.buf[0..3] != b"\x1b[M" {
        return None;
    }

    let button = self.buf[3].saturating_sub(32);
    let col = self.buf[4].saturating_sub(33);
    let row = self.buf[5].saturating_sub(33);

    self.buf.drain(..6);

    let event = match button {
        0 => MouseEvent::LeftPress { x: col, y: row },
        3 => MouseEvent::LeftRelease { x: col, y: row },
        32 | 64 => MouseEvent::WheelUp,
        1 | 65 => MouseEvent::WheelDown,
        _ => return Some(InputEvent::Mouse(MouseEvent::LeftRelease { x: col, y: row })),
    };

    Some(InputEvent::Mouse(event))
}
```

---

## 8. 边界情况处理

### 8.1 CJK 宽字符部分选择

- **问题**: 用户可能只选择 CJK 字符的右半列
- **解决**: `TextMap` 将左右两列映射到同一个 `CellSource`，提取时完整复制该字符

### 8.2 Line Wrap 边界

- **问题**: 选择跨越 line wrap 的逻辑行边界
- **解决**: `TextMap` 正确记录每个单元格的 `byte_offset`，提取时按 span 拼接，不插入额外换行符

### 8.3 ANSI 颜色代码

- **问题**: `blocks` 中包含 ANSI 颜色信息
- **解决**: 只复制纯文本，提取时遍历 `Line.spans`，拼接 `Span.text`，忽略 `fg/bg/bold/italic`

### 8.4 窗口 Resize

- **问题**: 窗口大小改变导致坐标失效
- **解决**: resize 时清除 `SelectionState`，状态栏提示：`窗口已调整，选择已清除`

### 8.5 滚动后选择

- **问题**: 滚动后再选择，坐标变化
- **解决**: `TextMap` 每帧重新构建，始终使用当前屏幕坐标

---

## 9. 实现优先级

### Phase 1: 核心选择功能（MVP）

**目标**: 实现基本的选择和反色渲染

- [ ] 新增 `SelectionState` 和增强的 `MouseEvent`
- [ ] 实现拖拽选择和反色渲染（使用屏幕坐标，暂不映射到文本）
- [ ] Ctrl+C 触发复制（简单实现：打印选择区域的坐标范围到 stderr）

**验收标准**:
- 鼠标拖拽能看到反色选择区域
- 按 Ctrl+C 在 stderr 看到选中的坐标范围（如 `Selection: (40,10)-(60,15)`）

### Phase 2: 文本提取与映射

**目标**: 从屏幕坐标正确提取文本内容

- [ ] 实现 `TextMap` 结构和构建逻辑
- [ ] 修改 `Widget::render` 签名，增加 `map: &mut TextMap` 参数
- [ ] 实现 `extract_selection_text()`
- [ ] 正确处理 CJK 宽字符、line wrap、ANSI 剥离

**验收标准**:
- 选中单行消息，Ctrl+C 输出正确文本
- 选中跨行消息，文本正确拼接（无额外换行符）
- 选中 CJK 字符（如"你好"），完整复制

### Phase 3: 剪贴板集成（单平台）

**目标**: 实现系统剪贴板复制

- [ ] 实现 Linux X11 FFI（`libX11.so`）
- [ ] 错误处理：连接失败时降级到 noop + 警告
- [ ] 状态栏提示：`已复制到剪贴板` / `无法复制：{原因}`

**验收标准**:
- Ctrl+C 后能在系统其他应用中粘贴
- X11 连接失败时有友好错误提示

### Phase 4: 跨平台扩展

**目标**: 支持 macOS 和 Windows

- [ ] macOS: AppKit framework FFI
- [ ] Windows: user32.dll FFI
- [ ] Linux Wayland: Wayland protocol over Unix socket

**验收标准**:
- 在 macOS 上能正常复制
- 在 Windows 上能正常复制

### Phase 5: 体验优化

**目标**: 提升选择效率

- [ ] 双击选择单词
- [ ] 三连选择整行
- [ ] Shift+点击扩展选择
- [ ] 滚动时自动清除选择

**验收标准**:
- 双击能选中完整单词（包括 CJK）
- 三连能选中整行
- Shift+点击能扩展/收缩选择区

---

## 10. 测试策略

### 10.1 单元测试

```rust
#[cfg(test)]
mod tests {
    // 选择区域标准化
    #[test]
    fn test_selection_normalize() {
        // 任意方向的选择都能正确标准化为左上角+右下角
    }

    // 坐标包含判断
    #[test]
    fn test_selection_contains() {
        // 边界情况：CJK 宽字符、部分选择、边界点
    }

    // 文本提取
    #[test]
    fn test_text_extraction() {
        // 跨块选择、line wrap、ANSI 剥离
        // CJK 字符完整性
    }
}
```

### 10.2 集成测试

```rust
// tests/tui/selection_test.rs

#[test]
fn test_full_selection_flow() {
    // 1. 创建 TerminalUI 实例
    // 2. 模拟鼠标事件序列（LeftPress → LeftDrag → LeftRelease）
    // 3. 验证 SelectionState 状态
    // 4. 验证渲染输出（buffer 内容）
    // 5. 验证提取的文本正确性
}
```

### 10.3 手动测试清单

- [ ] 基本拖拽选择（单行）
- [ ] 跨多行选择
- [ ] 跨消息块选择
- [ ] CJK 字符选择（"你好世界测试"）
- [ ] Ctrl+C 复制 + 系统粘贴验证
- [ ] 无选择时 Ctrl+C 中断 Agent
- [ ] 滚轮滚动后选择
- [ ] 窗口 resize 后选择状态清除
- [ ] 双击选择单词
- [ ] 三连选择整行
- [ ] Shift+点击扩展选择

---

## 11. 性能考虑

### 11.1 TextMap 构建开销

- **问题**: 每帧构建 `TextMap` 可能有开销
- **解决**:
  - 只在有选择时构建（`is_dragging || has_selection()`）
  - 使用 `HashMap` 稀疏存储，只为有文本的单元格记录

### 11.2 反色遍历开销

- **问题**: 遍历整个选择区域可能有开销
- **解决**:
  - 选择区域通常不大（几十到几百个单元格）
  - 遍历是简单的内存操作，开销可接受

### 11.3 文本提取开销

- **问题**: 提取时需要遍历 blocks 和 spans
- **解决**:
  - 提取只在 Ctrl+C 时触发，不是热路径
  - 使用 `HashSet` 和 `HashMap` 去重和排序，O(n log n)

---

## 12. 潜在风险与缓解

### 风险 1: FFI 实现复杂度高

- **影响**: X11/Wayland FFI 实现较复杂，容易出错
- **缓解**:
  - 先实现简单版本验证核心流程
  - 剪贴板返回 `Result`，失败时显示友好错误
  - 参考成熟实现（如 alacritty 的剪贴板代码）

### 风险 2: 终端兼容性

- **影响**: 某些终端可能不支持 1000h 模式或事件格式不同
- **缓解**:
  - 启动时检测鼠标支持，不可用时警告
  - 支持降级到无鼠标模式

### 风险 3: Ctrl+C 冲突

- **影响**: 用户可能在选中时想中断 Agent
- **缓解**:
  - 严格遵循"有选择=复制"逻辑
  - 状态栏显示提示：`[已选择 N 字符，Ctrl+C 复制]`
  - 提供其他中断方式（如 Esc 键）

### 风险 4: CJK 字符边界处理

- **影响**: 多字节字符的边界处理容易出错
- **缓解**:
  - 使用 `byte_offset` 而非 `char_idx`
  - 复制时用 `&text[start..end]` 切片，Rust 保证 UTF-8 边界正确

---

## 13. 未来扩展

### 13.1 矩形选择

- 支持列模式选择（Alt+拖拽）
- 对代码块特别有用

### 13.2 多选区

- 支持多个不连续的选择区
- Ctrl+点击添加到选择区

### 13.3 选择历史

- 记住最近的选择区
- Ctrl+Shift+Z 恢复上次选择

### 13.4 Yank 寄存器

- 类似 vim 的寄存器系统
- `"` + `a` 到 `"` + `z` 命名寄存器

---

## 附录 A: 参考资料

- [X11 Clipboard Specification](https://www.x.org/releases/X11R7.7/doc/xorg-docs/specs/ICCCM/icccm.html)
- [Wayland Protocol](https://wayland.freedesktop.org/)
- [alacritty clipboard implementation](https://github.com/alacritty/alacritty)
- [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html)

---

## 附录 B: 术语表

- **SGR (Select Graphic Rendition)**: ANSI 转义序列，用于设置文本样式（颜色、粗体等）
- **1000h / 1006h**: XTerm 的鼠标模式控制序列，1000=basic tracking, 1006=SGR encoding
- **Line wrap**: 长行自动换行显示，但不包含实际换行符
- **CJK 宽字符**: 中文、日文、韩文字符，在终端中占用 2 个显示列
- **FFI (Foreign Function Interface)**: Rust 调用其他语言（通常是 C）库的接口
