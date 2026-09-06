# 线格式与字节级规范（wire-format）

状态：v1。事实基线：docs/design/wire-protocol.md（v1）+ crates/ 代码；
冲突以代码为准。本文所有十六进制示例已用独立解码脚本验证往返一致。

## 1. 分层模型

一条连接自外向内四层，上层字节封装在下层字节流之内（wire-protocol.md 第 1 节）：

| 层 | QUIC 路径 | TCP 路径 |
|---|---|---|
| 传输 | QUIC 数据报套（TLS1.3 内建加密） | TCP（nodelay） |
| 安全 | TLS1.3 双向身份证书，随 QUIC 握手完成 | Noise XX 握手 + 密文记录层 |
| 复用 | QUIC 原生双向流 | yamux（拨号方固定 client 角色） |
| 流语义 | 首帧协议 ID + 后续业务帧（本文第 6-10 节） | 同左 |

两路产出统一的已完成握手连接：对端 PeerId 已互认（SecureConn.remote，
crates/p2p-transport/src/lib.rs:47-51）。复用层之上每条逻辑流独立使用本文帧格式；
一条流自始至终只承载一个协议。

## 2. 传输双路与默认端口

- 协议层面无固定端口；库默认端口 0（由操作系统随机分配，crates/p2p/src/lib.rs:57-59）。
- 部署约定端口（crates/p2p-cli/src/cli.rs:36-40,120-124，官方引导节点一致采用）：
  QUIC 3400/udp、TCP 3401/tcp、观测反射 3402/udp、中继 3403/udp + 3404/tcp。
- 节点应同时监听 QUIC 与 TCP；对端按其地址集逐个尝试，任一路可达即可。

## 3. 传输地址语法

展示与配置格式（crates/p2p-transport/src/lib.rs:13-27）：

    IP/u端口   -> QUIC（UDP）      例：203.0.113.10/u3400
    IP/t端口   -> TCP              例：203.0.113.10/t3401

可路由性判定（同文件 :29-44）：loopback（127.0.0.0/8、::1）只在本机有效；
IPv4 链路本地 169.254.0.0/16 与 IPv6 链路本地 fe80::/10 因缺接口作用域，
在注册/发现语义下不可拨；私网地址保留（同 NAT 直连是合法用途）。

## 4. 身份与安全握手

### 4.1 PeerId 推导（crates/p2p-identity/src/lib.rs:18-34）

    PeerId = base58( SHA-256( ed25519 公钥原始 32 字节 ) )

对原始公钥直接取哈希，无 multihash 前缀；内部为 32 字节定长摘要。
握手即认证：对端 PeerId 一律从握手密码学材料推导，任何自报身份字符串不得采信。

### 4.2 QUIC 路径：TLS1.3 + 身份证书

出处 crates/p2p-security/src/tls.rs、tls_cert.rs、tls_verify.rs。

- 双方各出一张自签证书做双向认证（rustls 强制 CertificateVerify 验签）；
  仅 TLS1.3；ALPN 固定 `p2p-base/1`（tls.rs:14），协商不一致即握手失败。
- 证书携带私有扩展 OID 1.3.6.1.4.1.59015.1（tls_cert.rs:18），内容为原始 32 字节
  ed25519 公钥。校验规则（tls_verify.rs）：只认带此扩展的自签证书，不查 CA；
  且证书 SPKI 公钥必须与扩展内容逐字节一致——扩展声明身份、SPKI 承担验签。
- 对端 PeerId = base58(SHA-256(扩展内公钥))。
- SNI 用固定占位符 `p2p-base`，不做域名语义（crates/p2p-transport/src/quic.rs:19）；
  QUIC keepalive 10s（:21），空闲超时 30s（:23），握手超时 10s（:25）。

### 4.3 TCP 路径：Noise XX

出处 crates/p2p-security/src/noise.rs。

- 参数串 `Noise_XX_25519_ChaChaPoly_SHA256`（:22）；XX 模式双方交换静态钥，无明文阶段。
- X25519 静态钥由同一 ed25519 身份派生：私钥 = SHA512(种子) 前 32 字节做 RFC 7748
  clamp；公钥 = ed25519 公钥转蒙哥马利坐标（:128-145）。
- 三条握手消息（发起方视角）：msg1 发 e；msg2 收 e,s,密文负载；msg3 发 s,密文负载。
- 身份负载定长 96 字节 = 32 字节 ed25519 公钥 || 64 字节 ed25519 签名（:163-172）；
  签名内容 = 域串 `p2p-noise-xx-v1`（:26）拼接本端 X25519 静态公钥。
  长度不符或验签失败即身份不可信，断链（:175-193，长度校验在 :176）。
- 握手帧与传输态记录同格式：2 字节大端长度 + 密文（u16be(len) || ciphertext，:205,227-239）；
  记录层之上再跑 yamux。握手超时默认 10s（:24）。

### 4.4 期望 PeerId 校验与不匹配语义

拨号时可指定期望 PeerId（expected）。握手完成后推导出的对端 PeerId 与期望不一致
即终止连接，错误 PeerMismatch（crates/p2p-transport/src/lib.rs:85-87，
文案形如 `peer mismatch: expected X, got Y`）。按 PeerId 拨号的实现必须带期望值：
地址来源（mDNS/rendezvous）可被污染，期望校验是身份锚点。
传输层错误三分类：Dial（地址/网络原因）、Handshake（握手失败）、PeerMismatch。

## 5. 流复用

- 单连接双向流上限 64（crates/p2p-mux/src/lib.rs:73，QUIC 传输参数与复用层
  信号量双重防护）；第 65 条流开不出来，实现方应转新连接或复用既有连接。
- 每条逻辑流独立按本文帧格式收发；一条流只承载一个协议，多协议并发 = 多条流。
- QUIC 流由 QUIC 原生承载；TCP 流经 yamux 复用，拨号方固定 client 角色，
  双方都可主动开流。

## 6. 帧格式（crates/p2p-protocol/src/lib.rs:142-222）

流语义层的全部字节按帧封装：**无符号 varint 长度前缀 + 定长 payload**。

    +------------------------------+======================================+
    | len: unsigned varint, 1-10 B | payload: len 字节（len <= 1 MiB）    |
    +------------------------------+======================================+

- varint 编码：LEB128，每字节低 7 位承载数据、最高位为后续标志（小端分组）；
  最多 10 字节，超出即 varint overflow 错误（:202-222）。第 10 字节只剩 1 个
  有效数据位，数据位 > 1 或仍带继续位都拒绝，不做静默回绕。
- 单帧上限 MAX_FRAME_SIZE = 1 048 576（1 MiB，:29）。
- 写侧：编码前先查长度，超限报 FrameTooLarge，不写出任何字节（:147-152）。
- 读侧：先读长度前缀再查，超限即报错断流，不预读 payload（:157-164）；
  长度与实际到字节不符按 IO 错误断流。
- 帧无类型字段、无版本字段：一帧的含义完全由所在流（协议 ID）与位置决定。

varint 编码样例（已验证）：

| 值 | 字节序列 |
|---|---|
| 0 | 00 |
| 127 | 7f |
| 128 | 80 01 |
| 300 | ac 02 |
| 16384 | 80 80 01 |
| 1048576 | 80 80 40 |

溢出样本：`80 80 80 80 80 80 80 80 80 02`（10 字节，末字节数据位为 2）
必须判 varint overflow 并断流，不得解析为任何值。

## 7. 协议 ID 语法（crates/p2p-protocol/src/lib.rs:36-62）

    "/" <segment> ("/" <segment>)* "/" <digits>

- 必须以 / 开头；去掉开头 / 后按 / 切分至少 2 段。
- 最后一段是版本段：非空纯 ASCII 数字（如 1、2、10 合法；v1、空串不合法，纯零串如 00 语法上合法但不应使用）。
- 其余段：非空，仅小写字母、数字、-、_（大写、空格、点不合法）。
- 等价正则：`/([a-z0-9_-]+/)+[0-9]+`（整串匹配）。

| 正例 | 反例（原因） |
|---|---|
| /p2p-base/rendezvous/1 | p2p-base/rendezvous/1（无前导斜杠） |
| /myapp/chat/2 | /p2p-base/rendezvous（无版本段） |
| /my_app/echo-1/10 | /p2p-base/rendezvous/v1（版本段非纯数字） |
| /dsh-acp/1 | /p2p base/rendezvous/1（段内空格） |
| /repair/mcp/1 | /MyApp/echo/1（大写） |

## 8. 新流开手顺序（crates/p2p-protocol/src/handshake.rs）

    发起方                                        接收方
      1. 在既有连接上开一条逻辑流
      2. 首帧 = 协议 ID 的 UTF-8 字节            1. 读首帧，按 UTF-8 解析为协议 ID
         （普通帧封装，见第 6 节）               2. 查本地 handler 注册表：
      3. 冲刷后即可写业务帧                         命中 -> 把流交给 handler
      4. 之后每帧一个业务 payload                  未命中 -> 关流，上报
                                                      UnsupportedProtocol

- 协议 ID 帧的 payload 就是 ID 字符串本身的 UTF-8 字节（lib.rs:170-182）；
  非 UTF-8 或语法非法按 InvalidData 断流。
- UnsupportedProtocol 语义 = 关流并上报事件，不做版本协商、不做猜测降级
  （handshake.rs:24,54）。发起方收到流被立即关闭即应视作"对端不支持"。

时序示例（/myapp/echo/1 一问一答）：

    A: [帧: len=13 "/myapp/echo/1"] [帧: len=5 "hello"]
    B: 读首帧 -> 查表命中 -> handler 收到流（已剥首帧）
    B: [帧: len=5 "hello"]
    A: 读回应；两侧 handler 返回即关流

## 9. request-response 原语（crates/p2p-protocol/src/request_response.rs）

每次请求开一条新流：开流 -> 写协议 ID -> 写请求帧 -> 读一帧回应 -> 关流；
一个超时覆盖全程，任一环节卡住即报 Timeout（:43-46），不悬挂。
请求与回应都是普通帧（第 6 节），请求 payload <= 1 MiB。
大消息先分帧（第 10 节）再作为单条"逻辑消息"交互。

## 10. chunked 传输（crates/p2p-protocol/src/chunked.rs）

单帧装不下的整条消息分帧传输，仍走第 6 节帧封装，帧 payload 首字节为类型头：

| 类型头 | 值 | 语义 |
|---|---|---|
| FRAME_SINGLE | 0x00 | 整条消息一帧装下，到此结束 |
| FRAME_CHUNK | 0x01 | 中间分片，后随更多帧 |
| FRAME_END | 0x02 | 最后一个分片，读端收到即重组完成 |

- 每帧数据部分上限 CHUNK_DATA_SIZE = 1 048 575（= 帧上限 - 1 字节类型头，:22）。
- 重组总大小防御上限 MAX_MESSAGE_SIZE = 67 108 864（64 MiB，:24），超限报
  MessageTooLarge；类型序非法（如分片中途出现 SINGLE、SINGLE 后还有帧）报 InvalidData。
- 切分规则：消息 > CHUNK_DATA_SIZE 时按 CHUNK_DATA_SIZE 逐片 FRAME_CHUNK，
  最后剩余字节（可为空）以 FRAME_END 收尾。

## 11. 字节级示例（已用独立脚本验证编解码往返）

### 例 A：协议 ID 首帧（/p2p-base/ping/1，16 字节 payload）

    payload = 2f 70 32 70 2d 62 61 73 65 2f 70 69 6e 67 2f 31   ("/p2p-base/ping/1")
    varint(16) = 10
    完整帧  = 10 2f 70 32 70 2d 62 61 73 65 2f 70 69 6e 67 2f 31   (17 字节)

解码：读 varint 得 16，再读 16 字节按 UTF-8 解析为协议 ID，校验语法（第 7 节）。

### 例 B：业务请求帧（payload "hi"）

    payload = 68 69
    varint(2) = 02
    完整帧  = 02 68 69

### 例 C：chunked 单帧（FRAME_SINGLE + "hello"）

    payload = 00 68 65 6c 6c 6f          (类型头 0x00 + 5 字节数据)
    varint(6) = 06
    完整帧  = 06 00 68 65 6c 6c 6f

### 例 D：chunked 多帧（"abcd" 分两帧）

    帧1 = 03 01 61 62     (len=3: 类型头 0x01 + "ab")
    帧2 = 03 02 63 64     (len=3: 类型头 0x02 + "cd")
    重组 = 61 62 63 64    ("abcd")

### 例 E：非法协议 ID 帧（/p2p-base/ping/v1）

    完整帧 = 11 2f 70 32 70 2d 62 61 73 65 2f 70 69 6e 67 2f 76 31
    解码出的 ID 版本段为 v1（非纯数字）-> InvalidData 断流，不进 handler。

## 12. 错误速查

| 错误 | 触发点 | 语义 |
|---|---|---|
| FrameTooLarge | 写侧编码前 / 读侧读出长度后 | 断流；写侧不写任何字节 |
| varint overflow | 读侧长度前缀 > 10 字节或第 10 字节非法 | 断流，拒绝静默回绕 |
| InvalidData | 协议 ID 非 UTF-8 / 语法非法 / chunked 类型序非法 | 断流 |
| UnsupportedProtocol | 收端注册表未命中协议 ID | 关流并上报，不降级 |
| Timeout | request-response 全程超时 | 可区分的显式错误 |
| MessageTooLarge | chunked 重组超 64 MiB | 断流 |
| PeerMismatch | 握手推导 PeerId != 期望值 | 连接终止 |
