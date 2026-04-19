# Phase 2: ASN.1/DER 解析器

## 背景

TLS 1.2 路线图 Phase 2。为 Phase 4（X.509 证书解析）和 Phase 5（ECDSA 签名解析）提供 ASN.1/DER 解码层。

## 目标与非目标

**目标**
- 解析 DER 编码的 TLV（Tag-Length-Value）结构
- 覆盖 X.509 证书所需的 ASN.1 类型
- 零拷贝：所有值返回借用 `&'a [u8]`，生命周期绑在输入缓冲上
- 零依赖、禁止崩溃代码

**非目标**
- **Encode**（写 DER）——Phase 4-7 只读不写；mTLS/CertificateVerify 已从精简路线图砍掉
- **BER** indefinite length——DER 禁用；输入若出现 0x80 length 直接报错
- **SET OF 乱序检查**——DER 要求按编码值递增；X.509 实际 SET 只出现在 RDN，parse 时不强校验
- **OID 到点号字符串的转换**——Phase 4 直接以 DER bytes 匹配预编译常量

## Tag 表示（语义拆解）

```rust
pub enum TagClass {
    Universal,       // 0b00
    Application,     // 0b01
    ContextSpecific, // 0b10
    Private,         // 0b11
}

pub struct Tag {
    pub class: TagClass,
    pub constructed: bool,
    pub number: u32,
}

impl Tag {
    // Universal primitive
    pub const BOOLEAN: Tag;           // class=Universal, constructed=false, number=1
    pub const INTEGER: Tag;           // number=2
    pub const BIT_STRING: Tag;        // number=3
    pub const OCTET_STRING: Tag;      // number=4
    pub const NULL: Tag;              // number=5
    pub const OID: Tag;               // number=6
    pub const UTF8_STRING: Tag;       // number=12
    pub const PRINTABLE_STRING: Tag;  // number=19
    pub const IA5_STRING: Tag;        // number=22
    pub const UTC_TIME: Tag;          // number=23
    pub const GENERALIZED_TIME: Tag;  // number=24
    pub const BMP_STRING: Tag;        // number=30

    // Universal constructed
    pub const SEQUENCE: Tag;          // constructed=true, number=16  (encodes as 0x30)
    pub const SET: Tag;               // constructed=true, number=17  (encodes as 0x31)

    // Context-specific helpers
    pub fn context(number: u32, constructed: bool) -> Tag;

    // Encoding/decoding to/from byte(s).
    // from_bytes reads one or more tag bytes from `input`, returning the Tag
    // and number of bytes consumed. Short form (number < 31) consumes 1 byte;
    // high-tag-number form consumes more.
    pub fn from_bytes(input: &[u8]) -> Result<(Tag, usize)>;
    // Matches this Tag against a single byte's short-form encoding. Returns
    // None for high-tag-number tags (number ≥ 31).
    pub fn to_short_byte(&self) -> Option<u8>;
}
```

高 tag number 形式（≥31）罕见，解码支持（`from_bytes`），编码不实现。

## Parser API

```rust
pub struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn remaining(&self) -> &'a [u8];

    // 读一个 TLV，返回 (tag, value)。value 是借用切片。
    pub fn read_any(&mut self) -> Result<(Tag, &'a [u8])>;

    // 读并断言 tag
    pub fn read_expect(&mut self, expected: Tag) -> Result<&'a [u8]>;
    pub fn peek_tag(&self) -> Result<Tag>;

    // 构造型：进入内部得到嵌套 parser（内容借用原 data）
    pub fn read_sequence(&mut self) -> Result<Parser<'a>>;
    pub fn read_set(&mut self) -> Result<Parser<'a>>;
    pub fn read_explicit(&mut self, context_number: u32) -> Result<Parser<'a>>;

    // 可选字段：下一个 TLV 的 tag 若匹配则读，否则留着
    pub fn read_optional(&mut self, tag: Tag) -> Result<Option<&'a [u8]>>;
    pub fn read_optional_explicit(&mut self, context_number: u32) -> Result<Option<Parser<'a>>>;

    // Primitive 解码（失败时 pos 不回退，失败即整张证书作废）
    pub fn read_bool(&mut self) -> Result<bool>;
    pub fn read_integer(&mut self) -> Result<&'a [u8]>;       // DER-encoded INTEGER payload (signed 2s-complement; Phase 3 会自己解)
    pub fn read_null(&mut self) -> Result<()>;
    pub fn read_oid(&mut self) -> Result<&'a [u8]>;            // raw OID bytes
    pub fn read_bit_string(&mut self) -> Result<BitString<'a>>;
    pub fn read_octet_string(&mut self) -> Result<&'a [u8]>;
    pub fn read_utf8_string(&mut self) -> Result<&'a str>;
    pub fn read_printable_string(&mut self) -> Result<&'a str>;
    pub fn read_ia5_string(&mut self) -> Result<&'a str>;
    pub fn read_utc_time(&mut self) -> Result<&'a str>;        // raw ASCII, Phase 4 会解析日期
    pub fn read_generalized_time(&mut self) -> Result<&'a str>;

    /// 断言已消费完所有输入。顶层 Parser 在解码完整 DER 后调用。
    pub fn finish(self) -> Result<()>;
}

pub struct BitString<'a> {
    /// Number of unused bits in the final byte (0-7).
    pub unused_bits: u8,
    /// Raw bytes. To read as a plain byte string, ignore unused_bits (for X.509
    /// signature values and public keys, unused_bits is always 0).
    pub bytes: &'a [u8],
}
```

## 长度编码

- Short form：`0xxxxxxx`（0–127）→ 单字节长度
- Long form：`1nnnnnnn`（n 为 1–4 个后续字节）→ 后续字节拼成 big-endian 长度
- 拒绝：`0x80`（indefinite length，BER only）
- 长度解码结果若超过 `data.len() - pos` → `Error::Asn1("truncated TLV")`

## 错误处理

新增 `Error::Asn1(String)`，所有失败走 `Result`：
- 输入截断
- 未预期的 tag（带上期望 vs 实际）
- 长度自洽不对
- indefinite length
- UTF-8 解码失败
- 嵌套 parser 未消费完（`read_sequence` 里的子 parser 在调用 `finish()` 时才校验）

顶层便捷：`parser.finish()` 断言已消费完输入，否则报错。

## 测试（`tests/core/asn1_test.rs`）

**Tag 编解码**
- `Tag::from_byte(0x02)` == `Tag::INTEGER`
- `Tag::from_byte(0x30)` == `Tag::SEQUENCE`
- `Tag::from_byte(0xA0)` → context, constructed, number=0
- 高 tag number 多字节编码解码

**长度解码**
- short form: 0x05 → 5
- long form 1-byte: 0x81, 0xff → 255
- long form 2-byte: 0x82, 0x01, 0x00 → 256
- 拒绝 indefinite length (0x80)
- 拒绝长度超缓冲

**Primitive 读取**
- INTEGER: `02 01 05` → `[0x05]`
- NULL: `05 00` → `()`
- OID: `06 03 2a 86 48`（= 1.2.840 首段）
- UTF8String: `0c 05 68 65 6c 6c 6f` → `"hello"`
- BIT STRING with unused bits: `03 04 06 01 23 45` → unused=6, bytes=[0x01,0x23,0x45]
- UTCTime: `17 0d 39 33 30 39 31 33 31 36 34 35 30 30 5a` → `"930913164500Z"`
- BOOLEAN: `01 01 ff` → `true`; `01 01 00` → `false`

**构造型**
- SEQUENCE 解嵌套：`30 06 02 01 01 02 01 02` → 内含两个 INTEGER (1, 2)
- `read_explicit(0)`：`A0 03 02 01 05` → INTEGER 5
- `read_optional(Tag::BOOLEAN)`：存在返回 Some，不存在返回 None 且 pos 不动

**错误场景**
- 截断：`30 05 02 01 05`（声明 5 字节但只有 3）
- 错 tag：读 INTEGER 时遇到 OCTET_STRING
- indefinite length：`30 80 ...`

**真实证书片段（端到端 smoke）**
- 一段修剪版 Let's Encrypt ISRG Root X1 DER 的头部（~50 字节），验证能从 `Certificate → TBSCertificate → version` 顺利推进

## 文件产出

| 文件 | 作用 | 新增/修改 |
|---|---|---|
| `src/core/asn1.rs` | Parser + Tag | 新增 (~400 LOC) |
| `src/core/mod.rs` | 加 `pub mod asn1;` | 修改 |
| `src/error.rs` | 加 `Asn1(String)` 变体 + Display | 修改 |
| `tests/core/asn1_test.rs` | 单元测试 | 新增 (~250 LOC) |
| `tests/core/mod.rs` | 加 `mod asn1_test;` | 修改 |

## 验收

- `cargo build` 无 warning
- `cargo test --test core_tests asn1` 全绿
- 全量 `cargo test` 无回归
- `grep -r 'unwrap\|expect\|panic!\|unreachable!\|todo!\|unimplemented!' src/core/asn1.rs` 无命中

## 依赖下游

- **Phase 4（X.509）** 用 `Parser` 解证书结构，用 OID 常量匹配签名算法
- **Phase 5（ECDSA）** 用 `Parser` 解 `ECDSA-Sig-Value ::= SEQUENCE { r INTEGER, s INTEGER }`
- **Phase 3（RSA）** 将 `read_integer` 的 `&[u8]` 通过 `BigUint::from_bytes_be` 转成大数（INTEGER 的前导 0 字节会被 BigUint 规范化掉）
