# 自实现 TLS 1.3 Client — 替代 OpenSSL FFI

**日期**: 2026-04-18
**状态**: Draft
**目标**: 去掉 OpenSSL FFI 依赖，用纯 Rust 实现最小 TLS 1.3 Client，支持 HTTPS 访问

## 背景

viv 当前通过 FFI 调用系统 OpenSSL（libssl）实现 TLS。这与零依赖/AgentOS 愿景冲突。
本方案参考 rustls 架构，实现一个最小化的 TLS 1.3 纯 Rust Client。

## 范围

**做**:
- TLS 1.3 Client（RFC 8446）
- 密码套件: `TLS_AES_128_GCM_SHA256` (0x1301)
- 密钥交换: X25519 (ECDHE)
- 纯 Rust 密码学原语: SHA-256, HMAC-SHA256, HKDF, AES-128-GCM, X25519
- 同步 `TlsStream` + 异步 `AsyncTlsStream`，公开接口不变

**不做**:
- TLS 1.2 / 1.1 / 1.0
- 其他密码套件 (AES-256-GCM, ChaCha20-Poly1305)
- 其他密钥交换 (P-256, P-384, FFDHE)
- 证书验证（先 stub，后续迭代）
- PSK / Session Resumption
- Post-handshake KeyUpdate
- Client Authentication
- TLS Server 功能

## 文件结构

```
src/core/net/tls/
├── mod.rs            # TlsStream, AsyncTlsStream — 公开接口
├── record.rs         # TLS Record Layer — 分帧、AEAD 加解密
├── handshake.rs      # TLS 1.3 Client 握手状态机
├── key_schedule.rs   # HKDF 密钥调度链
├── codec.rs          # TLS 消息编解码
├── crypto/
│   ├── mod.rs        # getrandom() syscall
│   ├── aes_gcm.rs    # AES-128-GCM (AEAD)
│   ├── sha256.rs     # SHA-256 + HMAC-SHA256 + HKDF
│   └── x25519.rs     # X25519 ECDHE
└── x509.rs           # 证书解析 stub
```

删除:
- `src/core/net/tls.rs` (旧 OpenSSL 同步)
- `src/core/net/async_tls.rs` (旧 OpenSSL 异步)

## §1 密码学原语层 (`crypto/`)

最底层，零项目内依赖，可完全独立测试。

### `crypto/mod.rs` — 安全随机数

```rust
pub fn getrandom(buf: &mut [u8]) -> Result<(), Error>
```

Linux `syscall(SYS_getrandom, buf, len, 0)`，syscall number 318 (x86_64)。

### `crypto/sha256.rs` — SHA-256 全家桶

```rust
pub struct Sha256 { /* 内部状态: 8×u32 哈希值 + 64字节 block 缓冲 + 总长度 */ }

impl Sha256 {
    pub fn new() -> Self
    pub fn update(&mut self, data: &[u8])
    pub fn finish(self) -> [u8; 32]
    pub fn hash(data: &[u8]) -> [u8; 32]
    pub fn clone(&self) -> Self            // transcript fork 用
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32]
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32]
pub fn hkdf_expand(prk: &[u8], info: &[u8], out: &mut [u8])
```

实现参考: FIPS 180-4 (SHA-256), RFC 2104 (HMAC), RFC 5869 (HKDF)。

### `crypto/aes_gcm.rs` — AES-128-GCM

```rust
pub struct Aes128Gcm {
    round_keys: [[u8; 16]; 11],   // AES-128: 10轮 + 初始密钥
    h: [u8; 16],                   // GHASH 密钥 = AES(key, 0)
}

impl Aes128Gcm {
    pub fn new(key: &[u8; 16]) -> Self
    pub fn encrypt(&self, nonce: &[u8; 12], aad: &[u8], plaintext: &[u8], out: &mut [u8])
    pub fn decrypt(&self, nonce: &[u8; 12], aad: &[u8], ciphertext_and_tag: &[u8], out: &mut [u8]) -> Result<usize, Error>
}
```

内部组件:
- AES-128 核心: 密钥扩展 + SubBytes/ShiftRows/MixColumns/AddRoundKey, 10 轮
- GHASH: GF(2^128) 有限域乘法 + 累加
- GCM: CTR 模式加密 + GHASH 认证, nonce = 12 字节

参考: FIPS 197 (AES), NIST SP 800-38D (GCM)。

### `crypto/x25519.rs` — X25519 密钥交换

```rust
pub fn keypair() -> ([u8; 32], [u8; 32])
pub fn shared_secret(our_secret: &[u8; 32], their_public: &[u8; 32]) -> [u8; 32]
```

Curve25519 Montgomery ladder 标量乘法。GF(2^255-19) 用 5×51-bit limb 表示。
参考: RFC 7748。

## §2 Record Layer (`record.rs`)

加密管道 — 明文进、密文出。

### TLS 1.3 Record 格式

```
ContentType(1B) | 0x0303(2B) | Length(2B) | encrypted_payload | content_type(1B) | tag(16B)
```

TLS 1.3: 外层 ContentType 固定 `ApplicationData(0x17)`，真实类型加密在 payload 末尾。

### 接口

```rust
pub struct RecordLayer {
    encrypter: Option<RecordEncrypter>,
    decrypter: Option<RecordDecrypter>,
}

struct RecordEncrypter {
    aes: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

struct RecordDecrypter {
    aes: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

impl RecordLayer {
    pub fn write_plaintext(&self, content_type: u8, payload: &[u8], out: &mut Vec<u8>)
    pub fn write_encrypted(&mut self, content_type: u8, payload: &[u8], out: &mut Vec<u8>)
    pub fn read_record(&mut self, data: &[u8]) -> Result<(u8, Vec<u8>, usize), Error>
    pub fn install_encrypter(&mut self, key: [u8; 16], iv: [u8; 12])
    pub fn install_decrypter(&mut self, key: [u8; 16], iv: [u8; 12])
}
```

Nonce 构造: `nonce = iv XOR (0x00000000 || seq_as_u64_be)`，每个 record 后 seq++。

## §3 Key Schedule (`key_schedule.rs`)

RFC 8446 §7.1 密钥调度链，硬编码 SHA-256 + AES-128-GCM。

### 密钥派生链

```
HKDF-Extract(salt=0, ikm=0) = Early Secret          [无 PSK，全零输入]
  │
  ▼ Derive-Secret(., "derived", "")
HKDF-Extract(salt=., ikm=ecdhe_shared) = Handshake Secret
  │
  ├─► Derive-Secret(., "c hs traffic", hash(CH..SH)) → client_hs_secret
  │   ├─► expand_label(., "key", 16) → client_hs_key
  │   ├─► expand_label(., "iv", 12)  → client_hs_iv
  │   └─► expand_label(., "finished", 32) → client_finished_key
  │
  ├─► Derive-Secret(., "s hs traffic", hash(CH..SH)) → server_hs_secret
  │   ├─► expand_label(., "key", 16) → server_hs_key
  │   ├─► expand_label(., "iv", 12)  → server_hs_iv
  │   └─► expand_label(., "finished", 32) → server_finished_key
  │
  ▼ Derive-Secret(., "derived", "")
HKDF-Extract(salt=., ikm=0) = Master Secret
  │
  ├─► Derive-Secret(., "c ap traffic", hash(CH..SF)) → client_app_secret
  │   ├─► expand_label(., "key", 16) → client_app_key
  │   └─► expand_label(., "iv", 12)  → client_app_iv
  │
  └─► Derive-Secret(., "s ap traffic", hash(CH..SF)) → server_app_secret
      ├─► expand_label(., "key", 16) → server_app_key
      └─► expand_label(., "iv", 12)  → server_app_iv
```

### 接口

```rust
pub struct TrafficKeys {
    pub key: [u8; 16],
    pub iv: [u8; 12],
}

pub struct KeySchedule { /* early_secret, handshake_secret, master_secret, hs secrets */ }

impl KeySchedule {
    pub fn new() -> Self
    pub fn derive_handshake_secrets(&mut self, shared: &[u8; 32], hash: &[u8; 32]) -> (TrafficKeys, TrafficKeys)
    pub fn derive_app_secrets(&mut self, hash: &[u8; 32]) -> (TrafficKeys, TrafficKeys)
    pub fn client_finished_key(&self) -> [u8; 32]
    pub fn server_finished_key(&self) -> [u8; 32]
}
```

核心工具函数:

```rust
fn hkdf_expand_label(secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8])
fn derive_secret(secret: &[u8], label: &[u8], transcript_hash: &[u8; 32]) -> [u8; 32]
```

`hkdf_expand_label` info 编码: `length(2B) || len("tls13 "+label)(1B) || "tls13 " || label || len(context)(1B) || context`

## §4 握手状态机 (`handshake.rs`)

### 状态链

```
SendClientHello → ExpectServerHello → ExpectEncryptedExtensions
→ ExpectCertificate → ExpectCertificateVerify → ExpectFinished
→ SendClientFinished → Complete
```

### 实现

```rust
enum HandshakeState {
    SendClientHello,
    ExpectServerHello,
    ExpectEncryptedExtensions,
    ExpectCertificate,
    ExpectCertificateVerify,
    ExpectFinished,
    SendClientFinished,
    Complete,
}

pub struct Handshake {
    state: HandshakeState,
    random: [u8; 32],
    x25519_secret: [u8; 32],
    x25519_public: [u8; 32],
    transcript: Sha256,
    key_schedule: KeySchedule,
    server_name: String,
}
```

### 每个状态的操作

1. **SendClientHello**: 生成 random + x25519 密钥对，编码 ClientHello，transcript 开始累积
2. **ExpectServerHello**: 提取 server x25519 公钥，完成 ECDHE，派生 handshake keys，安装 server 解密器
3. **ExpectEncryptedExtensions**: 解析（已加密），更新 transcript
4. **ExpectCertificate**: 读取证书数据，更新 transcript，不验证（stub）
5. **ExpectCertificateVerify**: 读取签名，更新 transcript，不验证（stub）
6. **ExpectFinished**: 验证 server Finished（`hmac(finished_key, transcript_hash)`），派生 app keys
7. **SendClientFinished**: 生成 client verify_data，发送 Finished + ChangeCipherSpec，安装 app keys

## §5 消息编解码 (`codec.rs`)

### 编码（Client → Server）

```rust
pub fn encode_client_hello(random, session_id, server_name, x25519_pub, out)
pub fn encode_finished(verify_data, out)
pub fn encode_change_cipher_spec(out)
```

ClientHello extensions: `server_name`, `supported_versions`(0x0304), `supported_groups`(x25519),
`key_share`(x25519 公钥), `signature_algorithms`(ecdsa_secp256r1_sha256, rsa_pss_rsae_sha256 等)。

### 解码（Server → Client）

```rust
pub enum HandshakeMessage {
    ServerHello(ServerHello),
    EncryptedExtensions(EncryptedExtensions),
    Certificate(Certificate),
    CertificateVerify(CertificateVerify),
    Finished(Finished),
}

pub fn decode_handshake(data: &[u8]) -> Result<HandshakeMessage, Error>
```

解码策略: 只解析需要的字段，未知 extension 按 length 跳过。

## §6 整合层 (`mod.rs`)

### TlsStream (同步)

```rust
pub struct TlsStream {
    tcp: std::net::TcpStream,
    record: RecordLayer,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

impl TlsStream {
    pub fn connect(host: &str, port: u16) -> crate::Result<Self>
}
impl Read for TlsStream { ... }
impl Write for TlsStream { ... }
impl Drop for TlsStream { ... }  // close_notify
```

### AsyncTlsStream (异步)

```rust
pub struct AsyncTlsStream {
    tcp: AsyncTcpStream,
    record: RecordLayer,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

impl AsyncTlsStream {
    pub async fn connect(host: &str, port: u16) -> crate::Result<Self>
    pub async fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize>
    pub async fn write_all(&mut self, buf: &[u8]) -> crate::Result<()>
}
```

复用现有 `async_tls.rs` 的 `wait_readable`/`wait_writable` reactor 集成。

### 迁移策略

1. 新代码写在 `src/core/net/tls/` 目录
2. 老 `tls.rs` / `async_tls.rs` 保留到新实现测试通过
3. 调用方 (`llm.rs`, `tools/web.rs`) 零改动

## §7 测试策略

### 密码学原语 — RFC 标准测试向量

| 原语 | 测试向量来源 |
|------|------------|
| SHA-256 | NIST FIPS 180-4 |
| HMAC-SHA256 | RFC 4231 |
| HKDF | RFC 5869 |
| AES-128 | FIPS 197 |
| AES-128-GCM | NIST SP 800-38D |
| X25519 | RFC 7748 |

### Key Schedule — RFC 8448 测试向量

RFC 8448 提供了完整的 TLS 1.3 握手示例，包含每一步的中间值。
用这个验证 `hkdf_expand_label` 和整个密钥派生链。

### 集成测试

`--features full_test` 下实际连接 HTTPS 服务器（如 api.anthropic.com），验证端到端握手成功。

## 预估规模

| 模块 | 行数 |
|------|------|
| `crypto/sha256.rs` | ~200 |
| `crypto/aes_gcm.rs` | ~350 |
| `crypto/x25519.rs` | ~300 |
| `crypto/mod.rs` | ~30 |
| `key_schedule.rs` | ~120 |
| `record.rs` | ~200 |
| `codec.rs` | ~300 |
| `handshake.rs` | ~250 |
| `mod.rs` | ~300 |
| `x509.rs` | ~20 |
| **总计** | **~2100** |

## 后续迭代

1. X.509 证书验证（DER 解析 + 签名链验证 + 域名匹配）
2. ChaCha20-Poly1305 密码套件
3. Session Resumption (PSK)
4. KeyUpdate 支持
5. 跨平台: Windows (BCryptGenRandom), macOS (getentropy)
