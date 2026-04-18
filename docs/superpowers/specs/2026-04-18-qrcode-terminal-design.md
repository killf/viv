# QR Code 终端组件设计

> 零依赖 QR Code 编码器 + 终端 Widget，支持 UTF-8 文本输入，Unicode 半块字符渲染。

## 目标

为 viv 提供一个通用的 QR Code 生成和终端显示组件。调用方传入文本，Widget 在终端中渲染出可扫描的二维码。

## 非目标

- QR Code 解码（只做编码）
- 二进制数据输入（只支持 UTF-8 文本）
- 多纠错级别选择（固定 M 级 15%）
- 图片输出（只做终端字符渲染）

---

## 1. 编码管线

```
UTF-8 文本
  → 数据编码（Byte mode: 模式指示 + 字符计数 + 数据字节 + 终止符 + 填充）
  → 版本选择（根据数据长度，查表选最小 Version 1-40）
  → 纠错码生成（Reed-Solomon over GF(256)）
  → 数据 + 纠错交织排列
  → 矩阵构建（功能图案 + 数据位蛇形放置）
  → 掩码评估 + 应用（8 种掩码，选罚分最低）
  → 格式信息 + 版本信息写入
  → QrMatrix (bool 矩阵)
  → Unicode 半块字符渲染 → QrCodeWidget
```

固定参数：
- 纠错级别：M（15%）
- 编码模式：Byte（UTF-8 字节直接编码）
- Version：自动选择最小能容纳数据的版本

矩阵大小：Version N → (4N + 17) × (4N + 17)。Version 1 = 21×21，Version 40 = 177×177。

---

## 2. GF(256) 有限域

QR Code 的 Reed-Solomon 纠错基于 GF(256) 有限域运算。

不可约多项式：x^8 + x^4 + x^3 + x^2 + 1（0x11D）。

预计算查找表：
- `EXP_TABLE: [u8; 256]` — α^i 的值
- `LOG_TABLE: [u8; 256]` — 对数表（LOG_TABLE[EXP_TABLE[i]] = i）

运算：
- `mul(a, b) -> u8` — 乘法：`EXP_TABLE[(LOG_TABLE[a] + LOG_TABLE[b]) % 255]`，a 或 b 为 0 则返回 0
- `div(a, b) -> u8` — 除法：`EXP_TABLE[(LOG_TABLE[a] - LOG_TABLE[b] + 255) % 255]`
- `pow(a, n) -> u8` — 幂运算

实现量：约 50 行。

---

## 3. Reed-Solomon 编码器

```rust
pub fn rs_encode(data: &[u8], ecc_count: usize) -> Vec<u8>
```

算法：
1. 构造生成多项式 g(x) = (x - α^0)(x - α^1)...(x - α^(ecc_count-1))
2. 数据多项式乘以 x^ecc_count
3. 除以生成多项式，余数即为纠错码字

ecc_count 由版本和纠错级别 M 查 ISO 18004 Table 9 确定。

实现量：约 40 行。

---

## 4. 数据编码 + 版本选择

### 数据编码流程（Byte mode）

```
1. 模式指示符:    0100                     (4 bits)
2. 字符计数:      数据字节数               (Version 1-9: 8 bits, Version 10+: 16 bits)
3. 数据字节:      逐字节写入               (每字节 8 bits)
4. 终止符:        0000                     (4 bits, 或更少如已到容量)
5. 补齐到字节边界: 补 0
6. 填充字节:      交替 0xEC 0x11            (直到填满数据码字容量)
```

### 版本选择

```rust
pub fn select_version(data_byte_count: usize) -> Option<u8>
```

Version 1-40 在纠错 M 下的 Byte mode 容量是固定常量数组。遍历找第一个能容纳的版本。超过 Version 40 容量（2331 字节）返回 None。

### 数据块分组

大版本的数据分成多个块（block），每块独立 RS 编码，然后交织排列：
- 块数和每块码字数查表（ISO 18004 Table 9）
- 数据码字按块分配，每块追加各自的纠错码字
- 交织：逐列取各块的数据码字，再逐列取各块的纠错码字

---

## 5. 矩阵构建

### QrMatrix 数据结构

```rust
pub struct QrMatrix {
    pub size: usize,
    pub modules: Vec<Vec<bool>>,      // true = 黑色模块
    is_function: Vec<Vec<bool>>,      // true = 功能图案区域，不参与掩码
}
```

### 功能图案

按固定规则放置：

1. **定位图案 (Finder Pattern)** — 3 个 7×7 方块：左上 (0,0)、右上 (0, size-7)、左下 (size-7, 0)
2. **分隔符 (Separator)** — 定位图案周围 1 模块白边
3. **定时图案 (Timing Pattern)** — 第 6 行和第 6 列的黑白交替线
4. **校正图案 (Alignment Pattern)** — Version 2+ 才有，中心位置查表，5×5 同心方块
5. **暗模块** — 固定位置 (4 * version + 9, 8) 设为黑

### 数据放置

从矩阵右下角开始，每次取 2 列宽度，蛇形上下交替：
- 右列在右，左列在左
- 跳过第 6 列（定时图案）
- 只在 `is_function == false` 的位置放数据 bit
- bit 顺序：先高位后低位

### 掩码

8 种掩码模式（编号 0-7），每种根据 (row, col) 判断是否翻转：

```
0: (row + col) % 2 == 0
1: row % 2 == 0
2: col % 3 == 0
3: (row + col) % 3 == 0
4: (row/2 + col/3) % 2 == 0
5: row*col%2 + row*col%3 == 0
6: (row*col%2 + row*col%3) % 2 == 0
7: ((row+col)%2 + row*col%3) % 2 == 0
```

只对数据区（`is_function == false`）应用。

对 8 种掩码分别计算罚分（ISO 18004 罚分规则 1-4），选总分最低的。

### 格式信息

15 bits = 纠错级别(2 bits) + 掩码编号(3 bits) + BCH(10,5) 纠错码(10 bits)。

写入两个固定位置（定位图案旁）。XOR 掩码 0x5412。

### 版本信息

Version 7+ 才需要。18 bits = 版本号(6 bits) + BCH(18,6) 纠错码(12 bits)。

写入右上和左下两个 6×3 区域。

---

## 6. Widget 渲染

### Unicode 半块字符

每个终端字符表示上下 2 个模块：

```
上黑下黑 → █ (U+2588 FULL BLOCK)
上黑下白 → ▀ (U+2580 UPPER HALF BLOCK)
上白下黑 → ▄ (U+2584 LOWER HALF BLOCK)
上白下白 → ' ' (空格)
```

### QrCodeWidget

```rust
pub struct QrCodeWidget<'a> {
    data: &'a str,
}
```

方法：
- `pub fn new(data: &'a str) -> Self`
- `pub fn height(data: &str) -> u16` — `(matrix_size + quiet_zone*2 + 1) / 2`

实现 `Widget` trait：

```rust
fn render(&self, area: Rect, buf: &mut Buffer) {
    // 1. encode(data) → QrMatrix (如果编码失败，显示错误文本)
    // 2. 加 2 模块 quiet zone（QR 规范要求的白色边框）
    // 3. 居中在 area 中
    // 4. 逐对行取上下两个模块 → 选择半块字符写入 Buffer
    //    前景色 = 白色（亮模块），终端默认背景作为暗模块
}
```

### 颜色方案

半块字符需要同时设置 fg（上半部分）和 bg（下半部分）：

```
上黑下黑 → '▀' fg=black  bg=black   (或 '█' fg=black)
上黑下白 → '▀' fg=black  bg=white
上白下黑 → '▀' fg=white  bg=black
上白下白 → '▀' fg=white  bg=white   (或 ' ' bg=white)
```

统一用 `▀` 字符，通过 fg/bg 颜色组合区分 4 种状态。黑色 = Rgb(0,0,0)，白色 = Rgb(255,255,255)。

QR 区域（含 quiet zone）整体需要白色背景，确保在深色终端上也能正常扫描。

---

## 7. 文件组织

```
src/
├── qrcode/
│   ├── mod.rs          // 模块导出 + pub fn encode(&str) -> Result<QrMatrix>
│   ├── gf256.rs        // GF(256) 有限域：EXP/LOG 表 + mul/div/pow
│   ├── rs.rs           // Reed-Solomon 编码器：rs_encode(data, ecc_count)
│   ├── encode.rs       // 数据编码 + 版本选择 + 块分组交织
│   ├── matrix.rs       // QrMatrix 构建 + 功能图案 + 数据放置 + 掩码 + 格式/版本信息
│   └── tables.rs       // 常量表：版本容量、纠错参数、校正图案位置
└── tui/
    └── qrcode.rs       // QrCodeWidget 渲染层

tests/
├── qrcode/
│   ├── mod.rs
│   ├── gf256_test.rs   // 有限域运算正确性
│   ├── rs_test.rs      // RS 编码已知向量
│   ├── encode_test.rs  // 版本选择、数据编码
│   └── matrix_test.rs  // 矩阵尺寸、功能图案、完整编码验证
└── tui/
    └── qrcode_test.rs  // Widget 渲染、高度计算
```

核心编码逻辑在 `src/qrcode/`（独立于 TUI 框架），Widget 在 `src/tui/qrcode.rs`。

### 公开 API

```rust
// src/qrcode/mod.rs
pub fn encode(data: &str) -> Result<QrMatrix>

// src/tui/qrcode.rs
pub struct QrCodeWidget<'a> { ... }
impl Widget for QrCodeWidget<'_> { ... }
```

调用方只需 `QrCodeWidget::new("https://example.com").render(area, buf)`。
