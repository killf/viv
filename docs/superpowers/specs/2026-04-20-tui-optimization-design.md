# TUI 优化设计方案

**日期**: 2026-04-20
**目标**: 优化 viv TUI 的视觉效果、交互体验

---

## 1. 欢迎页逐行淡入动画

### 现状
`WelcomeWidget` 直接渲染，无动画效果。

### 方案
在 `TerminalUI` 中增加 `welcome_anim: Option<WelcomeAnimState>` 字段：

```rust
struct WelcomeAnimState {
    start: Instant,
    total_lines: u16,
}
```

- `ROW_DELAY_MS = 80ms`：每行间隔
- `FADE_DURATION_MS = 200ms`：淡入持续时间
- 第 n 行可见度：`clamp((elapsed - n * 80) / 200.0, 0.0, 1.0)`
- 动画完成后 `welcome_anim = None`，静态渲染

### 视觉实现
用亮度差异模拟淡入：
- 不可见行：`theme::DIM`（灰色）
- 可见行：逐步过渡到 `theme::TEXT`（白色）
- Logo 行始终全亮，只对右侧 info 行做淡入

---

## 2. 工具调用边框呼吸动画

### 现状
`ToolCallWidget` Running 时只有 "⚙ running..." 文字，颜色 dim，无视觉强调。

### 方案
在 `TerminalUI` 中检测 Running 状态的 tool，累加流逝时间，计算呼吸相位：

```rust
fn breath_alpha(elapsed_ms: u64) -> f32 {
    let phase = (elapsed_ms % 1000) as f32 / 1000.0 * 2.0 * std::f32::consts::PI;
    (phase.sin() * 0.5 + 0.5) // 0..1
}
```

- 焦点行背景色在 `Color::Rgb(15,15,25)` 和 `Color::Rgb(35,30,50)` 之间插值
- 周期 1 秒，平滑正弦呼吸

---

## 3. 输入历史（↑/↓ 遍历消息）

### 现状
`LineEditor` 的 ↑/↓ 只在多行编辑时上下移动行，无法浏览历史。

### 方案
在 `LineEditor` 中增加字段：

```rust
pub struct LineEditor {
    // ...existing fields
    history: Vec<String>,           // 所有已发送的用户消息
    history_idx: Option<usize>,    // None=当前输入, Some(n)=浏览第 n 条
    original: String,               // 切换历史时保存当前输入
}
```

### 行为
- **↑**：首次按 → 保存当前输入到 `original`，显示 `history[last]`；再次按 → `history_idx -= 1`
- **↓**：如果已到最后一条 → 恢复 `original`，清空输入
- **任意编辑操作**：立即退出历史浏览模式
- **提交**：正常提交，清空输入

### 交互细节
- `history_idx` 为 `None` 时按 ↑ 才保存当前输入并进入浏览
- `history_idx` 为 `Some(0)` 时再按 ↑ 停在第一条

---

## 4. 鼠标滚轮支持

### 现状
Event 系统不支持鼠标事件。

### 方案

#### 4.1 InputParser — 解析 SGR 鼠标序列

```rust
#[derive(Debug, Clone)]
pub enum MouseEvent {
    WheelUp,
    WheelDown,
    LeftPress,
    LeftRelease,
}

// CSI SGR 序列: ESC [ < N ; X ; Y (M 或 m)
// N < 0: 按钮事件, N >= 64: 滚轮
```

支持的序列：
| 序列 | 含义 |
|------|------|
| `\x1b[<0;x;yM` | 左键按下 |
| `\x1b[<0;x;ym` | 左键释放 |
| `\x1b[<64;x;yM` | 滚轮上 |
| `\x1b[<65;x;yM` | 滚轮下 |

#### 4.2 Event — 新增 Mouse 变体

```rust
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(TermSize),
    Tick,
}
```

#### 4.3 TerminalUI — 鼠标滚轮映射

```rust
Event::Mouse(MouseEvent::WheelUp) => {
    self.conversation_state.scroll_up(3);
}
Event::Mouse(MouseEvent::WheelDown) => {
    self.conversation_state.scroll_down(3);
}
```

---

## 5. k/j 滚动确认

### 现状
代码逻辑正确，需确认端到端工作正常。

### 行为
- `Escape`（非 busy 时）→ 进入 Browse 模式
- Browse 模式下 `k`/`j` → 上下滚动
- `g` → 回到顶部，`G` → 到底部
- `n` → 切换工具焦点
- `Enter` → 展开/折叠工具
- `Escape` → 退出 Browse 模式

---

## 实现顺序

1. **输入历史** — `LineEditor` 增加 history（最常用）
2. **鼠标支持** — `input.rs` + `events.rs` + `terminal.rs`
3. **欢迎页动画** — `TerminalUI` + `WelcomeWidget`
4. **工具调用呼吸动画** — `TerminalUI`
5. **k/j 滚动确认** — 验证测试

---

## 涉及文件

| 文件 | 改动 |
|------|------|
| `src/core/terminal/input.rs` | 新增 MouseEvent，解析 SGR 鼠标序列 |
| `src/core/event.rs` | Event::Mouse 变体 |
| `src/tui/terminal.rs` | 欢迎动画、工具呼吸、鼠标处理、输入历史 |
| `src/tui/welcome.rs` | 淡入动画支持 |
| `src/tui/line_editor.rs` | 输入历史（若拆分出来） |
