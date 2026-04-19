# Phase 5: P-256 + ECDSA 签名验证

## 背景

TLS 1.2 路线图 Phase 5。验证 ECDSA over P-256（secp256r1）签名，覆盖现代 TLS 1.2 服务器的 ECDSA 证书和 ServerKeyExchange 消息。许多 Cloudflare / Let's Encrypt ECDSA 签发的证书属于此类。

## 目标与非目标

**目标**
- `EcdsaPublicKey::from_spki` 解析 P-256 ECDSA 公钥的 SPKI
- `verify_ecdsa_sha256` / `verify_ecdsa_sha256_prehashed` 验证 P-256 + SHA-256 签名
- `BigUint::mod_inverse`（扩展欧几里得），供 ECDSA 与未来算法复用
- 零依赖，禁止崩溃代码

**非目标**
- **密钥生成 / 签名**：仅公钥操作
- **常量时间**：无秘密（公钥、签名、消息全公开）
- **其他曲线**：secp256k1、P-384、Curve25519/Ed25519、中国 SM2 等都不做
- **压缩点（0x02/0x03 前缀）**：Let's Encrypt / Cloudflare 等主流 CA 均用未压缩格式
- **wNAF 预计算表**：朴素 double-and-add 对 verify 性能足够
- **Montgomery 域**：Solinas quick reduction 对 P-256 已很快

## 支持的算法

- Hash：SHA-256（已有）
- ECDSA P-256 verify
- Public-key 格式：SPKI DER（id-ecPublicKey + secp256r1 + uncompressed point）
- 签名格式：ASN.1 `SEQUENCE { r INTEGER, s INTEGER }`

## 常数定义

```rust
/// P-256 prime: p = 2^256 − 2^224 + 2^192 + 2^96 − 1
const P: [u64; 4] = [
    0xffffffff_ffffffff, 0x00000000_ffffffff,
    0x00000000_00000000, 0xffffffff_00000001,
];

/// P-256 curve order n (also prime)
const N: [u64; 4] = [
    0xf3b9cac2_fc632551, 0xbce6faad_a7179e84,
    0xffffffff_ffffffff, 0xffffffff_00000000,
];

/// Curve coefficient b (a is fixed at -3)
const B: [u64; 4] = [
    0x3bce3c3e_27d2604b, 0x651d06b0_cc53b0f6,
    0xb3ebbd55_769886bc, 0x5ac635d8_aa3a93e7,
];

/// Generator G = (Gx, Gy)
const GX: [u64; 4] = [
    0xf4a13945_d898c296, 0x77037d81_2deb33a0,
    0xf8bce6e5_63a440f2, 0x6b17d1f2_e12c4247,
];
const GY: [u64; 4] = [
    0xcbb64068_37bf51f5, 0x2bce3357_6b315ece,
    0x8ee7eb4a_7c0f9e16, 0x4fe342e2_fe1a7f9b,
];
```

## API

### `BigUint::mod_inverse`

```rust
impl BigUint {
    /// Modular inverse via extended binary GCD.
    /// Returns `Some(x)` where `(self * x) mod modulus == 1`,
    /// or `None` when `gcd(self, modulus) != 1` or `modulus` is zero.
    pub fn mod_inverse(&self, modulus: &Self) -> Option<Self>;
}
```

算法选择：**Extended Euclidean**（常规版；binary version 更快但更易出错）。~80 LOC。

### `src/core/net/tls/p256.rs`

```rust
/// Field element in GF(p) where p = 2^256 − 2^224 + 2^192 + 2^96 − 1.
/// Stored as little-endian [u64; 4]; canonical form (< p).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldElement([u64; 4]);

impl FieldElement {
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn from_bytes_be(bytes: &[u8; 32]) -> Option<Self>;   // None if >= p
    pub fn to_bytes_be(&self) -> [u8; 32];

    pub fn add(&self, other: &Self) -> Self;                  // mod p
    pub fn sub(&self, other: &Self) -> Self;                  // mod p
    pub fn neg(&self) -> Self;
    pub fn mul(&self, other: &Self) -> Self;                  // mod p, Solinas reduction
    pub fn square(&self) -> Self;
    pub fn invert(&self) -> Option<Self>;                     // None if zero
}

/// Point in Jacobian coordinates (X:Y:Z); affine (x,y) = (X/Z², Y/Z³).
/// Z == 0 encodes the point at infinity.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: FieldElement,
    pub y: FieldElement,
    pub z: FieldElement,
}

impl Point {
    pub fn infinity() -> Self;
    pub fn generator() -> Self;

    /// Parse `0x04 || x(32) || y(32)` uncompressed point; validates y² = x³ − 3x + b.
    pub fn from_uncompressed(bytes: &[u8; 65]) -> crate::Result<Self>;

    pub fn is_infinity(&self) -> bool;
    pub fn is_on_curve(&self) -> bool;

    /// Convert to affine and return x as 32 BE bytes; None for infinity.
    pub fn affine_x_bytes(&self) -> Option<[u8; 32]>;

    pub fn double(&self) -> Self;
    pub fn add(&self, other: &Self) -> Self;
    pub fn neg(&self) -> Self;

    /// Scalar multiplication via left-to-right double-and-add.
    /// Scalar is 256-bit big-endian bytes, treated as integer.
    pub fn scalar_mul(&self, scalar: &[u8; 32]) -> Self;
}
```

### `src/core/net/tls/ecdsa.rs`

```rust
use crate::core::net::tls::p256::Point;

pub struct EcdsaPublicKey {
    pub point: Point,
}

impl EcdsaPublicKey {
    /// Parse SubjectPublicKeyInfo DER. Accepts only
    ///   algorithm = id-ecPublicKey (1.2.840.10045.2.1)
    ///   parameters = namedCurve prime256v1 (1.2.840.10045.3.1.7)
    ///   subjectPublicKey = BIT STRING of 0x04 || x(32) || y(32) (65 bytes)
    pub fn from_spki(der: &[u8]) -> crate::Result<Self>;
}

/// Verify ECDSA-SHA256 signature. `signature` is DER `SEQUENCE { r, s }`.
pub fn verify_ecdsa_sha256(
    pk: &EcdsaPublicKey,
    msg: &[u8],
    signature: &[u8],
) -> crate::Result<()>;

/// Same but with pre-computed SHA-256 digest.
pub fn verify_ecdsa_sha256_prehashed(
    pk: &EcdsaPublicKey,
    digest: &[u8; 32],
    signature: &[u8],
) -> crate::Result<()>;
```

## 验证算法（FIPS 186-4 §6.4）

```
Input: pk = Point Q, digest = [u8; 32] (SHA-256), signature DER SEQUENCE { r, s }

Step 1. Parse (r, s) from DER signature. Both as BigUint.
Step 2. Range check: 1 ≤ r < n, 1 ≤ s < n. (n is the curve order.)
Step 3. e = digest treated as big-endian integer, truncated to n's bit length
        (for P-256 n is 256 bits, same as SHA-256 output size; no truncation needed).
Step 4. w = s^(-1) mod n                    (BigUint::mod_inverse)
Step 5. u1 = e * w mod n                    (BigUint mul + div_rem for mod)
        u2 = r * w mod n
Step 6. u1_bytes = u1 to 32-byte BE
        u2_bytes = u2 to 32-byte BE
Step 7. P = u1·G + u2·Q                     (P-256 scalar_mul + point add)
Step 8. if P is infinity → Err("point at infinity")
Step 9. x_P = P.affine_x_bytes() as BigUint
        v = x_P mod n
        if v == r → Ok
        else → Err("signature mismatch")
```

## Solinas Reduction for P-256

P-256 prime has special form enabling fast reduction without division:

```
Given t = t_15||...||t_0 (16 × 32-bit limbs, representing a 512-bit number from a·b):

Per FIPS 186-4 D.2, compute:
  s1 = t_7  t_6  t_5  t_4  t_3  t_2  t_1  t_0
  s2 = t_15 t_14 t_13 t_12 t_11 0    0    0
  s3 = 0    t_15 t_14 t_13 t_12 0    0    0
  s4 = t_15 t_14 0    0    0    t_10 t_9  t_8
  s5 = t_8  t_13 t_15 t_14 t_13 t_11 t_10 t_9
  s6 = t_10 t_8  0    0    0    t_13 t_12 t_11
  s7 = t_11 t_9  0    0    t_15 t_14 t_13 t_12
  s8 = t_12 0    t_10 t_9  t_8  t_15 t_14 t_13
  s9 = t_13 0    t_11 t_10 t_9  0    t_15 t_14
  
  r = (s1 + 2·s2 + 2·s3 + s4 + s5 − s6 − s7 − s8 − s9) mod p
  
  Then add/subtract p until result in [0, p).
```

~50 LOC of bit shuffling. Trade some readability for avoiding BigUint-style division per multiplication.

## 错误处理

- 所有失败 `Err(Error::Tls(String))`，与 Phase 3/4 风格一致
- 标签化错误：`"ECDSA: sig length"`、`"ECDSA: r out of range"`、`"ECDSA: mod_inverse of s failed"`、`"ECDSA: point at infinity"`、`"ECDSA: x_P mismatch"` 等
- 无 `unwrap`/`panic` 路径

## 测试

**`tests/core/bigint_test.rs`** 追加 `mod_inverse` 测试：
- 小模数已知逆：`3^-1 mod 11 = 4`（因 3·4=12≡1）、`7^-1 mod 15 = 13`
- 不互素：`mod_inverse(4, 6)` == None
- `mod_inverse(0, 7)` == None
- 2048-bit 量级性能 smoke < 100ms

**`tests/core/net/tls/p256_test.rs`**：
- `FieldElement::from_bytes_be/to_bytes_be` 往返
- `FieldElement::mul`：NIST P-256 KAT 的几组向量
- `FieldElement::invert`：`x · x⁻¹ == 1`
- `Point::generator()::is_on_curve()` == true
- `G + G == 2G`（= `G.double()`）
- `G + infinity == G`
- `G + (-G) == infinity`
- 标量乘一致性：`2·G == G.double()`、`3·G == 2G + G`
- `from_uncompressed` 解析 generator 字节

**`tests/core/net/tls/ecdsa_test.rs`**：
- `from_spki` 解析 openssl 生成的 ECDSA 公钥
- 错 OID（如 rsaEncryption）→ Err
- 压缩点（0x02 前缀）→ Err
- 长度 ≠ 65 的 BIT STRING → Err
- `verify_ecdsa_sha256` 验真实签名 → Ok
- 篡改签名 → Err（多种位置：r 翻转、s 翻转、DER 破坏）
- 篡改消息 → Err
- r == 0 / s == 0 / r == n → Err
- 性能：单次 verify < 200ms

## 文件产出

| 文件 | 作用 | 新增/修改 |
|---|---|---|
| `src/core/bigint.rs` | 加 `mod_inverse` | 修改 (+~80 LOC) |
| `src/core/net/tls/p256.rs` | FieldElement + Point + 常量 | 新增 (~500 LOC) |
| `src/core/net/tls/ecdsa.rs` | 验证 + SPKI | 新增 (~200 LOC) |
| `src/core/net/tls/mod.rs` | 加 `pub mod p256; pub mod ecdsa;` | 修改 |
| `tests/core/bigint_test.rs` | mod_inverse 测试 | 修改 |
| `tests/core/net/tls/p256_test.rs` | P-256 测试 | 新增 (~200 LOC) |
| `tests/core/net/tls/ecdsa_test.rs` | ECDSA 测试 | 新增 (~200 LOC) |
| `tests/core/net/tls/mod.rs` | 加 2 个 mod | 修改 |

## 验收

- `cargo build` 无 warning
- `cargo test --test core_tests p256` / `ecdsa` / `mod_inverse` 全绿
- 全量 `cargo test` 无回归（除已知上游引入的 qrcode/llm 编译问题）
- `grep` src/ 无崩溃调用
- ECDSA verify 单次 < 200ms（朴素 double-and-add 实测可能 ~50ms）

## 依赖下游

- Phase 6：ECDSA 证书的链验证
- Phase 7：TLS_ECDHE_ECDSA_* cipher suite 下 ServerKeyExchange 签名验证
