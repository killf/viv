# Phase 3: RSA PKCS#1 v1.5 签名验证

## 背景

TLS 1.2 路线图 Phase 3。验证 RSA 公钥签名是建连 TLS 1.2 服务器与核对 X.509 证书链的必备能力。Phase 3 负责：
- 从 X.509 SubjectPublicKeyInfo 中解出 RSA 公钥 `(n, e)`
- 对 SHA-256 摘要 + 签名对验证 PKCS#1 v1.5 格式

## 目标与非目标

**目标**
- `RsaPublicKey::from_spki(der)` 解析 SPKI
- `verify_pkcs1_sha256(pk, msg, sig)` + `verify_pkcs1_sha256_prehashed(pk, &digest, sig)` 两个验证入口
- 零依赖，禁止崩溃代码

**非目标**
- **签名 / 加密**：只做公钥验证
- **RSA-PSS / OAEP**：TLS 1.2 RSA ServerKeyExchange 与大多数 X.509 证书都用 PKCS#1 v1.5；PSS 留给未来扩展
- **SHA-1 / SHA-384 / SHA-512**：现代证书链 >99% 用 SHA-256；若 Phase 6/7 在真实服务器上遇到别的 hash 再按需追加
- **MGF1 / 盐值**：PKCS#1 v1.5 不需要
- **常量时间**：公钥操作，无秘密

## 支持的算法

- Hash：SHA-256（已有 `src/core/net/tls/crypto/sha256.rs`）
- RSA PKCS#1 v1.5 verify
- 键长：2048 / 3072 / 4096 bits（Phase 1 BigUint 泛化支持）

## API

`src/core/net/tls/rsa.rs`：

```rust
use crate::core::bigint::BigUint;
use crate::Error;

pub struct RsaPublicKey {
    pub n: BigUint,
    pub e: BigUint,
}

impl RsaPublicKey {
    /// 从 SPKI（SubjectPublicKeyInfo DER）构造。
    /// SPKI 结构：
    ///   SubjectPublicKeyInfo ::= SEQUENCE {
    ///     algorithm AlgorithmIdentifier,   -- OID 必须是 rsaEncryption
    ///     subjectPublicKey BIT STRING       -- 内部 DER(RSAPublicKey)
    ///   }
    ///   RSAPublicKey ::= SEQUENCE {
    ///     modulus INTEGER,
    ///     publicExponent INTEGER
    ///   }
    pub fn from_spki(der: &[u8]) -> crate::Result<Self>;

    /// 从原始大端字节构造。调用方保证 n 与 e 都是合法的 RSA 公钥参数。
    pub fn from_n_e(n_be: &[u8], e_be: &[u8]) -> Self;

    /// Modulus 的字节长度（= 签名长度）。
    pub fn n_byte_len(&self) -> usize;
}

/// 验证 PKCS#1 v1.5 with SHA-256 签名。`msg` 未 hash，函数内部做 SHA-256。
pub fn verify_pkcs1_sha256(
    pk: &RsaPublicKey,
    msg: &[u8],
    signature: &[u8],
) -> crate::Result<()>;

/// 同上但接受预先算好的摘要（X.509 验证链会用）。
pub fn verify_pkcs1_sha256_prehashed(
    pk: &RsaPublicKey,
    digest: &[u8; 32],
    signature: &[u8],
) -> crate::Result<()>;
```

## 验证流程

```
Input: pk = (n, e), digest [u8;32], signature &[u8]

Step 1. 签名长度检查
  if signature.len() != pk.n_byte_len():
      return Err(Error::Tls("RSA: signature length mismatch"))

Step 2. 转成 BigUint
  s = BigUint::from_bytes_be(signature)

Step 3. 范围检查
  if s >= n:
      return Err(Error::Tls("RSA: signature out of range"))

Step 4. 模幂
  m = s.modexp(&e, &n)
  em = m.to_bytes_be(pk.n_byte_len())   -- 左补零到 k 字节

Step 5. 分步解析 EM (RFC 8017 §9.2)
  k = em.len()
  if k < 11 + 19 + 32:   -- 至少 3 + 8(PS) + 19(DigestInfo) + 32(hash)
      return Err(Error::Tls("RSA: EM too short for SHA-256"))
  if em[0] != 0x00:
      return Err(Error::Tls("RSA: EM[0] != 0x00"))
  if em[1] != 0x01:
      return Err(Error::Tls("RSA: EM[1] != 0x01 (block type)"))

  -- PS: 连续 0xFF
  let mut i = 2
  while i < k && em[i] == 0xFF { i += 1 }
  if i - 2 < 8:
      return Err(Error::Tls("RSA: PS shorter than 8 bytes"))
  if i >= k || em[i] != 0x00:
      return Err(Error::Tls("RSA: missing PS terminator 0x00"))
  i += 1

  -- DigestInfo prefix + digest
  let remaining = &em[i..]
  if remaining.len() != 19 + 32:
      return Err(Error::Tls("RSA: DigestInfo length mismatch"))
  if remaining[..19] != SHA256_DIGEST_INFO_PREFIX:
      return Err(Error::Tls("RSA: DigestInfo prefix mismatch"))
  if remaining[19..] != digest:
      return Err(Error::Tls("RSA: digest mismatch"))

  Ok(())
```

**常量：**

```rust
/// DER-encoded DigestInfo prefix for SHA-256:
///   SEQUENCE {
///     SEQUENCE {
///       OID 2.16.840.1.101.3.4.2.1 (sha256),
///       NULL
///     },
///     OCTET STRING (32 bytes) -- digest follows
///   }
const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
];

/// OID 1.2.840.113549.1.1.1 rsaEncryption (raw DER content bytes).
const OID_RSA_ENCRYPTION: [u8; 9] = [
    0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
];
```

## 错误策略

- 失败统一返回 `Err(Error::Tls(String))`
- 消息字符串区分：length、range、EM[0]、EM[1]、PS、terminator、prefix、digest
- 无 panic 路径；无 unwrap；所有数组索引前做 bounds check

## 测试策略

`tests/core/net/tls/rsa_test.rs`（~200 LOC）

**从 openssl 离线生成，硬编码为 hex 常量：**

```bash
# 生成一次，不在 CI 执行；把结果粘贴到测试文件
openssl genrsa -out test_rsa2048.pem 2048
openssl rsa -in test_rsa2048.pem -pubout -outform DER -out test_rsa2048_pub.der
echo -n "hello world" > test_msg.bin
openssl dgst -sha256 -sign test_rsa2048.pem -out test_sig.bin test_msg.bin
# 把 test_rsa2048_pub.der, test_msg.bin, test_sig.bin 的 hex 粘到测试
```

同类 tuple 生成 3 组：{2048-bit, 3072-bit, 4096-bit} × 一组消息。

**测试清单：**

1. **SPKI 解析**
   - `from_spki(known_2048_spki_bytes)` 成功，`n_byte_len() == 256`, `e` == 65537
   - 错 OID（如 ecdsa-with-SHA256） → Err
   - BIT STRING unused_bits != 0 → Err
   - SEQUENCE 截断 → Err

2. **正向验证**
   - 2048-bit：`verify_pkcs1_sha256(pk, msg, sig)` → Ok
   - 3072-bit 同理
   - 4096-bit 同理
   - `verify_pkcs1_sha256_prehashed` 用同一组向量

3. **负向验证（对签名或摘要逐位翻转）**
   - 签名长度不对：截断 1 字节 → Err("length mismatch")
   - 签名置全零 → modexp 出 0 → EM 全零 → Err("EM[0]")
   - 翻转签名中间一位 → Err（具体类别视修改位置，测只断言 is_err）
   - 修改 msg 一字节 → hash 不同 → Err("digest mismatch")

4. **边界**
   - s = n（等于模数）→ Err("out of range")
   - s = 0 → EM 全零 → Err("EM[0]")

5. **性能 smoke**
   - 2048-bit verify 单次 < 100ms（Phase 1 实测 ~10ms；带 100ms 留 CI 余量）

## 文件产出

| 文件 | 作用 | 新增/修改 |
|---|---|---|
| `src/core/net/tls/rsa.rs` | `RsaPublicKey` + verify | 新增 (~250 LOC) |
| `src/core/net/tls/mod.rs` | `pub mod rsa;` | 修改 |
| `tests/core/net/tls/rsa_test.rs` | 测试 | 新增 (~200 LOC) |
| `tests/core/net/tls/mod.rs` | `pub mod rsa_test;` | 修改 |

## 验收

- `cargo build` 无 warning
- `cargo test --test core_tests rsa` 全绿
- 全量 `cargo test` 无回归
- `grep` src/ 无崩溃调用
- 2048-bit verify < 100ms

## 依赖下游

- Phase 4（X.509）：调用 `RsaPublicKey::from_spki` 从 Certificate.tbsCertificate.subjectPublicKeyInfo 提取公钥
- Phase 6（证书链验证）：`verify_pkcs1_sha256_prehashed` 用 issuer 公钥验证每张证书的签名
- Phase 7（TLS 1.2 协议）：验证 ServerKeyExchange 的 RSA 签名
