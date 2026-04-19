# Phase 4: X.509 证书解析

## 背景

TLS 1.2 路线图 Phase 4。解析服务器发来的 X.509 v3 DER 编码证书，为 Phase 6（证书链验证）提供结构化字段。

## 目标与非目标

**目标**
- `X509Certificate<'a>::from_der(der)` 零拷贝解析证书
- 暴露链验证所需的最小字段：tbs_bytes, issuer_dn, subject_dn, validity, spki, san_dns_names, is_ca, signature, signature_algorithm
- 主机名匹配（含 RFC 6125 wildcard）与时间有效期检查
- 替换现有的 `x509.rs` 空壳（TLS 1.3 的 `verify_chain`/`verify_signature` 无操作 stub）

**非目标**
- **链验证与签名验证**：Phase 6
- **OCSP / CRL / CT log**：精简路线图外
- **DN 规范化**：issuer/subject 按 raw DER bytes 比较（DER 已经确定性编码；现代 CA 全用 UTF8String）
- **SAN 的 iPAddress / URI / rfc822Name / etc.**：只解 dNSName
- **证书策略 / 名称约束 / Policy Mapping 等复杂扩展**
- **非 UTC 时间**：只支持尾 `Z`（UTC）
- **keyUsage / extendedKeyUsage 验证**：精简范围外（Phase 6 若需要再加）

## 字段与类型

```rust
// src/core/net/tls/x509.rs

pub struct X509Certificate<'a> {
    pub raw: &'a [u8],               // full DER blob
    pub tbs_bytes: &'a [u8],          // TBSCertificate (tag+length+value) — the signed message
    pub version: u32,                 // 0=v1, 1=v2, 2=v3
    pub serial: &'a [u8],
    pub signature_algorithm: &'a [u8], // outer algorithm identifier OID (raw)
    pub issuer_dn: &'a [u8],          // raw Name SEQUENCE bytes (tag+length+value)
    pub subject_dn: &'a [u8],         // raw Name SEQUENCE bytes
    pub not_before: DateTime,
    pub not_after: DateTime,
    pub spki: &'a [u8],               // full SubjectPublicKeyInfo DER
    pub san_dns_names: Vec<&'a str>,  // empty if no SAN extension
    pub is_ca: Option<bool>,          // None if no basicConstraints extension
    pub signature: &'a [u8],          // outer BIT STRING content (no unused-bits prefix)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
```

**API：**

```rust
impl<'a> X509Certificate<'a> {
    pub fn from_der(der: &'a [u8]) -> crate::Result<Self>;

    /// Hostname match per RFC 6125 §6.4.3 wildcard rules (leftmost-label only).
    /// Case-insensitive; returns true on any SAN match.
    pub fn matches_hostname(&self, hostname: &str) -> bool;

    /// Inclusive validity check: returns true if `now` ∈ [notBefore, notAfter].
    pub fn is_valid_at(&self, now: &DateTime) -> bool;
}

impl DateTime {
    /// Current UTC time from `SystemTime::now`.
    pub fn now_utc() -> Self;

    /// Parse 13-char `YYMMDDHHMMSSZ` (UTCTime).
    /// Year mapping per RFC 5280 §4.1.2.5.1: YY < 50 → 20YY, YY ≥ 50 → 19YY.
    pub fn from_utc_time(s: &str) -> crate::Result<Self>;

    /// Parse 15-char `YYYYMMDDHHMMSSZ` (GeneralizedTime).
    pub fn from_generalized_time(s: &str) -> crate::Result<Self>;
}
```

## 结构解析

```
Certificate ::= SEQUENCE {
    tbsCertificate     TBSCertificate,
    signatureAlgorithm AlgorithmIdentifier,
    signatureValue     BIT STRING
}

TBSCertificate ::= SEQUENCE {
    version         [0] EXPLICIT Version DEFAULT v1,
    serialNumber    INTEGER,
    signature       AlgorithmIdentifier,   -- must equal outer signatureAlgorithm
    issuer          Name,
    validity        SEQUENCE { notBefore Time, notAfter Time },
    subject         Name,
    subjectPublicKeyInfo SubjectPublicKeyInfo,
    issuerUniqueID  [1] IMPLICIT BIT STRING OPTIONAL,   -- skip if present
    subjectUniqueID [2] IMPLICIT BIT STRING OPTIONAL,   -- skip if present
    extensions      [3] EXPLICIT Extensions OPTIONAL
}

Extensions ::= SEQUENCE OF Extension
Extension ::= SEQUENCE {
    extnID    OID,
    critical  BOOLEAN DEFAULT FALSE,
    extnValue OCTET STRING              -- inner ext payload DER
}
```

## 关键扩展

**SubjectAltName** (OID 2.5.29.17)：

```
SubjectAltName ::= GeneralNames
GeneralNames ::= SEQUENCE SIZE (1..MAX) OF GeneralName
GeneralName ::= CHOICE {
    otherName                [0] ...,
    rfc822Name               [1] IA5String,
    dNSName                  [2] IA5String,     -- only this
    x400Address              [3] ...,
    directoryName            [4] ...,
    ediPartyName             [5] ...,
    uniformResourceIdentifier [6] IA5String,
    iPAddress                [7] OCTET STRING,
    registeredID             [8] OID
}
```

只收 `[2] IA5String` dNSName；其他 tag 一律跳过（非错误）。

**BasicConstraints** (OID 2.5.29.19)：

```
BasicConstraints ::= SEQUENCE {
    cA                BOOLEAN DEFAULT FALSE,
    pathLenConstraint INTEGER (0..MAX) OPTIONAL
}
```

只取 `cA` 值，`pathLenConstraint` 暂不暴露。

**未知扩展策略**：无论 `critical` 位是否为 true，统统忽略。这不符合 RFC 5280 严格语义（critical=true 的未知扩展应导致拒绝），但对"能访问大多数站点"目标足够，且避免了主流 CA 使用 AKI/CT poison 等 critical 扩展被误拒的风险。Phase 6 若需更严格可以后加。

## 时间解析

**UTCTime** `YYMMDDHHMMSSZ`：

```
if len(s) != 13 || s[12] != 'Z':  → Err
parse YY MM DD hh mm ss as decimal digits, each 2-char:
    YY:  00..49 → year = 2000 + YY
         50..99 → year = 1900 + YY
validate: 1 ≤ MM ≤ 12, 1 ≤ DD ≤ 31 (不做 day-of-month 精确校验), 0 ≤ hh ≤ 23, 0 ≤ mm ≤ 59, 0 ≤ ss ≤ 60 (60 允许闰秒)
```

**GeneralizedTime** `YYYYMMDDHHMMSSZ`：同上但 4 位年、15 字符。

**当前时间** `now_utc()`：用 `std::time::SystemTime::now()` + `duration_since(UNIX_EPOCH)` 换算。现有 `src/log/mod.rs:125` 的 `rata_die` 算法可以复用（或直接拷贝到 `x509.rs`；精简路线图允许重复代码）。

## 主机名匹配（RFC 6125 §6.4.3 简化）

```
matches(san_entry, hostname):
    san_lc = san_entry.to_ascii_lowercase()
    host_lc = hostname.to_ascii_lowercase()
    # Non-wildcard: direct equality
    if !san_lc.starts_with("*."):
        return san_lc == host_lc
    # Wildcard: leftmost label is `*`, rest must match exactly
    suffix = &san_lc[1..]  # includes the leading '.'
    # Wildcard cannot match multi-label: count '.' in host_lc that precede suffix
    if !host_lc.ends_with(suffix):
        return false
    host_prefix_len = host_lc.len() - suffix.len()
    if host_prefix_len == 0:
        return false  # wildcard cannot match empty
    # No dots in the prefix (single-label substitution)
    return !host_lc[..host_prefix_len].contains('.')
```

例：`*.example.com`
- `a.example.com` → true
- `a.b.example.com` → false
- `example.com` → false
- `.example.com` → false（prefix 为空）

## Parser 扩展

需要在 `src/core/asn1.rs` 加一个方法：

```rust
impl<'a> Parser<'a> {
    /// Read one TLV and return the complete raw bytes (tag+length+value).
    /// Useful for `tbs_bytes`: the signature covers tag+length+value.
    pub fn read_any_raw(&mut self) -> crate::Result<&'a [u8]>;
}
```

实现：记录 `pos_before`，调用 `read_any()`，返回 `&self.data[pos_before..self.pos]`。

## 错误处理

- 所有失败 `Err(Error::Tls(String))`，无 panic
- 解析过程中保持 `Parser::finish()` 习惯，剩余字节检测防截断

## 测试

`tests/core/net/tls/x509_test.rs`：

- 主测试向量：本地 openssl 生成的自签 2048-bit RSA 证书，`CN=test.example.com`, SAN=`DNS:test.example.com, DNS:*.example.com`
- plan 里会给出完整 DER hex（类似 Phase 3 的做法）
- 测试项：
  1. `from_der` 全部字段正确解析
  2. 主机名匹配：精确、wildcard、多层拒绝、不相关拒绝
  3. 时间有效期：过去 / 现在 / 未来
  4. `is_ca == Some(false)` for leaf
  5. SPKI bytes 能喂给 `RsaPublicKey::from_spki`
  6. 截断 DER → Err
  7. UTCTime 年份边界 (YY=49 vs YY=50)
  8. GeneralizedTime 解析
  9. DateTime 比较顺序

## 文件产出

| 文件 | 作用 | 新增/修改 |
|---|---|---|
| `src/core/net/tls/x509.rs` | 整体替换：`X509Certificate` + `DateTime` + parsers | 修改 (~500 LOC) |
| `src/core/asn1.rs` | 加 `read_any_raw` | 修改 (+~15 LOC) |
| `tests/core/net/tls/x509_test.rs` | 单元测试 | 新增 (~300 LOC) |
| `tests/core/net/tls/mod.rs` | 声明 `pub mod x509_test;` | 修改 |

## 验收

- `cargo build` 无 warning
- `cargo test --test core_tests x509` 全绿
- 全量 `cargo test` 无回归
- `grep` src/ 无崩溃调用
- X.509 解析单次 < 10ms（纯 Rust 无密码学运算，应 ~100µs 量级）

## 依赖下游

- Phase 6：链验证 (issuer_dn / tbs_bytes / signature / is_ca / is_valid_at / matches_hostname)
- Phase 7：TLS Certificate message 把一串 DER blob 喂给 `from_der`
