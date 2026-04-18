# Phase 1: BigUint（大整数运算）

## 背景

TLS 1.2 路线图（见 `2026-04-19-tls12-roadmap.md`）的第 1 阶段。为 Phase 3（RSA PKCS#1 v1.5 签名验证）和 Phase 5（P-256 + ECDSA）提供通用的无符号大整数类型。

## 目标与非目标

**目标**
- 提供 `BigUint` 类型，支持 RSA 2048/3072/4096 位的公钥验证
- 正确、易测、足够快（单次 RSA-2048 modexp <50ms）
- 零依赖，与现有 crypto 模块风格一致（u64 limb + u128 中间乘积）

**非目标**
- 常量时间——我们只做公钥操作，没有秘密要保护
- 带符号整数（BigInt）——RSA/ECDSA 验证不需要
- 性能极致（Karatsuba / Montgomery / Barrett）——schoolbook + Knuth Algorithm D 已够用
- 除法/乘法之外的高级操作（gcd, mod_inverse）——留给 Phase 5 按需扩展

## API

模块路径：`src/core/bigint.rs`（以 `crate::core::bigint::BigUint` 导出）

```rust
use std::cmp::Ordering;

/// 无符号大整数，小端 limb 表示。
///
/// 不变量：
/// - `limbs` 无尾随零（若最高 limb 存在则非零）
/// - 零值 = `limbs.is_empty()`（等价于 `vec![]`）
pub struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    // ── 构造 ──────────────────────────────────────────────────
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn from_u64(v: u64) -> Self;

    /// 从大端字节序构造。前导 0 被自动规范化。
    pub fn from_bytes_be(bytes: &[u8]) -> Self;

    // ── 序列化 ────────────────────────────────────────────────
    /// 输出大端字节序，长度为 `max(out_len, self.byte_len())`。
    /// 即 out_len 是"最小长度"，不足会左补 0；若实际需要的字节数超过 out_len
    /// 则返回实际长度（不截断）。RSA 验证时传入 modulus 的字节长度即可。
    pub fn to_bytes_be(&self, out_len: usize) -> Vec<u8>;

    pub fn bit_len(&self) -> usize;
    pub fn is_zero(&self) -> bool;

    // ── 比较 ──────────────────────────────────────────────────
    pub fn cmp(&self, other: &Self) -> Ordering;

    // ── 算术（包级可见，供 rsa.rs / ecdsa.rs 复用） ───────────
    pub(crate) fn add(&self, other: &Self) -> Self;
    pub(crate) fn checked_sub(&self, other: &Self) -> Option<Self>;  // None if self < other
    pub(crate) fn mul(&self, other: &Self) -> Self;

    /// 除法 + 取余。Divisor 为 0 时返回 None。
    pub(crate) fn div_rem(&self, divisor: &Self) -> Option<(Self, Self)>;

    // ── 密码学用公开 API ──────────────────────────────────────
    /// base^exp mod modulus，square-and-multiply 实现。
    /// modulus 为 0 时返回 None。
    pub fn modexp(&self, exp: &Self, modulus: &Self) -> Option<Self>;
}

// 常规 trait 实现
impl PartialEq for BigUint { ... }
impl Eq for BigUint {}
impl Clone for BigUint { ... }
impl std::fmt::Debug for BigUint { ... }  // 16 进制输出
```

## 算法实现

| 操作 | 算法 | 复杂度 |
|---|---|---|
| `add` | schoolbook，u128 进位 | O(n) |
| `checked_sub` | schoolbook，借位检测 | O(n) |
| `mul` | schoolbook（外层 n × 内层 n） | O(n²) |
| `div_rem` | Knuth Algorithm D（TAOCP Vol 2 §4.3.1） | O(n²) |
| `modexp` | 左到右 square-and-multiply，每步 `mul` 后 `div_rem` 取模 | O(bits(exp) · n²) |

对 RSA-2048、e=65537（17 bits）：~17 次平方 + 1 次乘法 = ~18 次模乘，每次模乘 ≈ 2048² bit 的乘法 + 2048 bit 除法，总耗时估算 <5 ms。

**不变量维护**：每次算术后都规范化（剥尾零），保证 `cmp` / `bit_len` / `is_zero` 简单正确。

## 错误处理

- `checked_sub`：返回 `Option<BigUint>`（`None` 表示 `self < other`）
- `div_rem`、`modexp`：返回 `Option<...>`（`None` 表示除/模数为 0）
- `add`、`mul`：不会失败，返回 `BigUint`
- 不使用 `unwrap`/`expect`/`panic`/`unreachable`，符合项目"禁止崩溃代码"约束

## 测试策略

测试文件：`tests/core/bigint_test.rs`

**基础**
1. 零值、一值、`from_u64` 的构造与序列化
2. `from_bytes_be`：前导 0 的规范化（`[0, 0, 1, 2]` → `0x0102`）
3. `to_bytes_be` 往返（含左补 0 边界：`BigUint::one().to_bytes_be(32)` = `[0u8;31] + [1]`）
4. `cmp`：相等、大于、小于、不同位宽

**算术**
5. `add`：进位跨 limb 边界
6. `checked_sub`：`None` 场景、借位跨 limb 边界
7. `mul`：对照值（2^128 × 2^128 = 2^256 等）
8. `div_rem`：
   - RFC 8017 附录 A.2.1 的 2048-bit 模数 N 与典型签名 s 的 `s mod N`
   - Knuth Algorithm D 的边界：商的试除修正

**modexp**
9. 小向量：`2^10 mod 1000 = 24`、`3^7 mod 11 = 9`
10. NIST CAVP 公开的 RSA-2048 验签向量（至少 3 组）
11. e=65537、n=2048 的"假"公钥上跑 modexp，耗时 <50ms（给慢 CI 机留余量）
12. modulus = 0 → `None`；exp = 0 → `one`；base = 0 → `zero`

**规范化不变量**（内部白盒测试）
13. 结果 `limbs` 不含尾随 0（反射用 `#[cfg(test)]` 暴露的辅助函数检查）

## 文件产出

- 新增：`src/core/bigint.rs`（~400 LOC）
- 新增：`tests/core/bigint_test.rs`（~200 LOC）
- 修改：`src/core/mod.rs` 加 `pub mod bigint;`
- 修改：`tests/core/mod.rs` 加 `mod bigint_test;`

## 验收

- `cargo build` 通过，无 warning
- `cargo test --test core_tests bigint` 全绿
- 所有测试单项 <1s；`modexp` 性能断言 <50ms
- 源码 grep 确认无 `unwrap/expect/panic/unreachable`

## 依赖下游 Phase 的说明

- **Phase 3（RSA）** 会调用 `BigUint::modexp`、`from_bytes_be`、`to_bytes_be`
- **Phase 5（ECDSA）** 会需要 `mod_inverse`（扩展欧几里得）——Phase 5 的 spec 会按需扩展 `BigUint`，不在本 phase 范围
