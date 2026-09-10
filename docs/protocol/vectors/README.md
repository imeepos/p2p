# 符合性测试向量（Conformance Vectors）

本目录是 p2p-base 全部协议的**唯一向量源**（单源双用）：

- 第三方实现者：按本 README 的 schema 用任意语言消费，验证你的实现与 p2p-base 互通语义一致，
  不需要阅读本仓库源码；
- 本仓库：`crates/p2p-conformance` 读同一批 JSON 断言自身实现（防回归），并含消融测试
  （翻转向量任一字节必须变红）。

文件名清单由 `docs/protocol/spec-charter.md` §8 冻结，规范页 §7 按文件名引用，不得改名。

## 1. JSON 顶层统一形状

每个 `*.json` 都是这样一个对象：

```json
{
  "vector_set": "<向量集名称，与本文件名（去 .json）一致>",
  "spec": "<语义依据文档（协议 ID / wire-format.md 章节 / 实现出处）>",
  "cases": [ { "name": "...", "note": "...", "...": "该向量集自定义字段" } ]
}
```

规则：

- `cases` 内每个 case **必须**含 `name`（集内唯一，snake_case）与 `note`（人读语义说明）；
  其余字段由各向量集自定义，见下表。
- 字节一律用**小写十六进制字符串**，紧凑无分隔。
- 大整数（超过 2^53）一律用**十进制字符串**表示（如 `"18446744073709551615"`），
  避免 JavaScript 等语言双精度精度丢失；字段名后缀或描述会注明单位。
- `valid: true` 的 case 是黄金样例（实现必须复现/接受）；`valid: false` 是负样例
  （实现必须拒绝，`error` 字段给出语义错误类名，不同语言的报错文案不作要求）。
- 描述性键（如 `limits`、`frame_note`、`payload_layout`）是集级元数据，不属于任何 case。

## 2. 各向量集 case 字段

### varint.json — 无符号 LEB128 varint

| 字段 | 含义 |
|---|---|
| `value` | 被编码数值（十进制字符串，合法样例才有） |
| `bytes_hex` | varint 编码字节 |
| `valid` / `error` | 溢出拒绝样例：`varint_overflow` |

语义：最多 10 字节；编码必须最短；解码必须拒绝第 10 字节仍带继续位、或第 10 字节数据位 >1
的输入（防 `<<63` 高位丢弃回绕）。

### frame.json — 帧封装（varint 长度前缀 + payload）

| 字段 | 含义 |
|---|---|
| `payload_hex` / `frame_hex` | 完整帧字节 = 前缀 + payload |
| `declared_len` + `payload_repeat_hex` | 上限边界样例：按字节重复展开 payload（避免向量文件存 1 MiB） |
| `frame_prefix_hex` | 仅长度前缀（超限负样例线上无需真实字节） |
| `available_payload_hex` | 截断样例：声明长度大于实际字节 |
| `valid` / `error` | `frame_too_large`（长度 > 1048576）、`unexpected_eof`（长度与实际不符必须断流） |

### peer-id.json — PeerId 推导全链

| 字段 | 含义 |
|---|---|
| `seed_hex` | 32 字节 ed25519 种子 |
| `public_key_hex` | ed25519 公钥（32 字节） |
| `sha256_hex` | SHA-256(公钥) 中间值（32 字节，调试用） |
| `peer_id_base58` | base58(sha256 摘要)，即 PeerId 的规范字符串形式 |

### chunked.json — 分帧传输（SINGLE 0x00 / CHUNK 0x01 / END 0x02）

| 字段 | 含义 |
|---|---|
| `frames` | 每项是一个帧的 payload（类型头+数据）；线上每帧外再套 varint 长度前缀（同 frame.json） |
| `message_hex` | 重组后完整消息（合法样例） |
| `eof_after_frames` | true 表示该帧序列之后流被截断 |
| 重组上限样例 | `chunk_data_repeat_hex`+`chunk_data_len`+`chunk_frames` 描述 N 个满载 CHUNK，`end_data_*` 描述收尾 END 帧 |
| `valid` / `error` | `invalid_type_sequence`（SINGLE 出现在分片中途）、`invalid_type_header`（类型头不在 0x00-0x02）、`missing_type_header`（空帧）、`truncated`（END 前断流）、`message_too_large`（累计 > 67108864 字节） |

集级 `limits`：`chunk_data_size=1048575`（单帧数据上限）、`max_message_size=67108864`。
重组上限用 64×1048575+65 的等价构造表达，无需真实传输 64 MiB。

### rendezvous-register.json — /p2p-base/rendezvous/1 签名注册

| 字段 | 含义 |
|---|---|
| `seed_hex` / `public_key_hex` / `peer_id_base58` | 固定种子身份链 |
| `namespace` / `addrs` / `ttl_secs` / `issued_at` | 注册字段（`issued_at` 为十进制字符串 unix 秒） |
| `signed_fields_hex` | 被签字节：protobuf `{namespace=1, peer_id=2, addrs=3(repeated AddrMsg), ttl_secs=4, issued_at=5}`；`AddrMsg {quic=1(bool), ip=2(string), port=3(uint32)}` |
| `signature_hex` | ed25519 签名（覆盖 `signed_fields_hex`，64 字节） |
| `register_protobuf_hex` | 线上 Register 消息完整编码：protobuf `{namespace=1, peer_id=2, pubkey=3, addrs=4(repeated AddrMsg), ttl_secs=5, sig=6, issued_at=7}` |
| 负样例 | `ttl_tamper_rejected`：改 ttl 重编码后验签必须失败；`freshness_window_rejected`：`verify_now` 距 `issued_at` 超 300 秒必须拒 |

### relay-messages.json — /p2p-base/relay/1 RelayMsg oneof tag 1-9

| 字段 | 含义 |
|---|---|
| `protobuf_hex` | RelayMsg protobuf 信封字节（去掉长度前缀） |
| `frame_hex` | 线上帧 = varint(长度) + `protobuf_hex` |

oneof tag：1 Reserve / 2 Reserved / 3 Connect / 4 Bound / 5 PunchReq / 6 PunchAck /
7 Reject / 8 KeepAlive / 9 KeepAliveAck。错误码表（Reject.code）1-7 样例齐备。
集级 `proto3_note`：零值标量不写入编码，解码缺字段按零值处理。

### im-chat-envelope.json — /im/chat/1 信封与回执

| 字段 | 含义 |
|---|---|
| `frame_type` / `type_byte` | ENVELOPE=0x01、ACK=0x04（MEDIA_BEGIN 0x02 / MEDIA_CHUNK 0x03 不在本集） |
| `payload_json` | 载荷 JSON（紧凑序列化） |
| `payload_hex` / `frame_hex` | 载荷字节与整帧字节 |
| MIME 样例 | `kind`/`mime`/`size`（`size` 为十进制字符串）三元组，`valid` 表白名单裁决 |

信封 JSON 字段（camelCase，固定序）：`id, peer, sender, kind, tsMs, text, media, replyTo,
card, fromAddrs`；`media = {name, mime, size}`；Option 缺省序列化为 `null`。
收端校验：`sender` 必须为 `"me"`（对端视角）、`peer` 不得指向本机。MIME 白名单（mime 小写后
精确匹配）：image: png/jpeg/gif/webp；audio: mpeg/wav/ogg/m4a/mp4；video: mp4/webm/mov/
quicktime；file/text 不限；groupinvite 空集（拒绝一切附件）。size 上限 67108864 且不得为 0。

### handshake-identity.json — 安全握手身份负载与 TLS 常量

| 字段 | 含义 |
|---|---|
| `seed_hex` → `public_key_hex` → `x25519_static_hex` | ed25519 种子、公钥、其蒙哥马利（X25519）映射 |
| `sign_domain_hex` | 域分隔串 `7032702d6e6f6973652d78782d7631`（`p2p-noise-xx-v1`） |
| `signed_message_hex` | 被签消息 = 域串 + X25519 静态公钥 |
| `signature_hex` / `identity_payload_hex` | 64 字节签名；96 字节负载 = 公钥[0..32) + 签名[32..96) |
| TLS 断言 case | `tls_identity_extension_oid`（OID `1.3.6.1.4.1.59015.1`，扩展内容=32 字节 ed25519 公钥且与 SPKI 一致）、`quic_alpn`（`p2p-base/1`） |

完整握手 transcript（QUIC/Noise 全程）属 Extended 候选，不在本集。

## 3. 第三方消费指引（任意语言）

1. **只依赖本目录**：JSON 是自包含的；不需要本仓库源码、容器或网络。所有样例确定性可复现
   （ed25519 同一种子同一消息必得同一签名）。
2. **推荐顺序**：先跑 `varint` + `frame`（传输底座），再 `peer-id` + `handshake-identity`
   （身份与安全），然后按你实现的协议跑对应向量集。
3. **黄金样例（valid: true）**：你的实现按语义构造输入后，输出字节必须逐字节一致；
   解码类样例（帧、消息、信封）则要求解析结果与向量字段语义一致。
4. **负样例（valid: false）**：你的实现必须显式失败（断流/回错误码/验签拒绝），
   不得静默吞掉后继续按正常状态机推进。
5. **重复字节 pattern**：`*_repeat_hex` + `*_len`/`*_frames` 表示把该 hex 字节重复指定次数/
   帧数后展开，避免向量文件内嵌兆级字节。
6. **十进制字符串大整数**：解析 `"18446744073709551615"` 这类值时用任意精度或 u64 原生整数，
   不要走双精度浮点。
7. **消融自查**：把黄金样例任一密码学字节（签名、被签消息）在内存翻转 1 位，你的校验必须变红；
   这是本目录对"校验真的在验"的最低要求（本仓 `p2p-conformance` 已内置同款测试）。

## 4. 本仓消费方式

`crates/p2p-conformance` 以 `CARGO_MANIFEST_DIR` 相对路径读取本目录 JSON（向量数据不复制进
Rust 源码），断言 crates/ 实现与向量一致；`cargo test -p p2p-conformance` 即全量回归。
