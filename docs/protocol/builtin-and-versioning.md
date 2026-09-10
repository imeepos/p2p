# 内置协议、登记流程与版本演进（builtin-and-versioning）

状态：v2（PROTO 轮全表刷新）。实现状态逐条以 crates/ apps/ 代码为准（2026-09-10）；
全表与 docs/protocol/registry.toml（schema 1）16 个公开协议 ID 逐条一致，与
docs/design/wire-protocol.md 第 3.2 节的核对结果见第 1.3 节。机器可读索引以
registry.toml 为准，本文是人读对齐视图。

## 1. 内置协议 ID 全表

### 1.1 底座控制协议（登记点：crates/p2p-relay/src/lib.rs:8-15 proto_ids 模块）

全底座唯一定义点，禁止重复定义；实现状态为本表核对实况：

| 协议 ID | 职责 | 实现状态 |
|---|---|---|
| /p2p-base/identify/1 | 交换公钥、监听地址、观测地址 | 常量已登记；无 handler 实现（规划中，未实现） |
| /p2p-base/ping/1 | 往返延迟探测、连通性保活 | 已实现：swarm 内置 PingHandler，读一帧原样回一帧；节点自动注入，用户同名注册优先（crates/p2p-swarm/src/swarm/ping.rs:31-62） |
| /p2p-base/rendezvous/1 | 签名注册/查询节点地址（带 TTL） | 已实现：客户端 + 服务端注册表（crates/p2p-discovery/src/rendezvous/） |
| /p2p-base/relay/1 | 中继电路申请、打洞信令控制流 | 已实现（crates/p2p-relay/src/control.rs） |
| /p2p-base/circuit/1 | 中继电路数据桥接（密文透传） | 已实现（crates/p2p-relay/src/circuit.rs、state.rs） |
| /dsh-acp/1 | ACP 桥握手 + ndjson 字节透传 | 已实现（apps/acp-common 共享面 + apps/acp-agent agent 侧端点；操作者侧 crates/acp-pump；规范页 specs/dsh-acp.md） |

### 1.2 已登记的业务协议（同一路由机制，无特权差别）

| 协议 ID | 归属 crate | 实现状态 |
|---|---|---|
| /im/chat/1 | crates/p2p-chat/src/wire.rs | 已实现（好友 1:1 私聊，一流一事务） |
| /im/group/1 | crates/p2p-chat/src/group_wire.rs | 已实现（群聊：消息/roster/踢出/退出席） |
| /im/invite/1 | crates/p2p-chat/src/wire_invite.rs | 已实现（邀请制加好友生命周期） |
| /im/ginvite/1 | crates/p2p-chat/src/ginvite_wire.rs | 已实现（同意制入群邀请，owner-only 发起） |
| /im/profile/1 | crates/p2p-chat/src/profile_wire.rs | 已实现（对端节点资料按需查询，纯读） |
| /llm-share/proxy/1 | crates/llm-share-proxy/src/wire.rs | 已实现（E10 额度共享代理） |
| /llm-share/offer/1 | crates/llm-share-offer | 已实现（能力声明发布） |
| /llm-share/redeem/1 | crates/llm-share-link | 已实现（分享链接兑换：请求 token，响应 ok/结构化拒绝码） |
| /a2a/1 | crates/a2a | 已实现（A2A 智能体：card 相发布/发现/订阅 + task 相 JSON-RPC，1 task=1 流） |
| /repair/mcp/1 | crates/repair-bridge/src/lib.rs:6 | 已实现（MCP stdio 字节隧道哑泵；规范页 specs/repair-mcp.md） |

测试专用 ID（/itest/echo/1、/p2p-lab/echo/1、/test/echo/1 等）仅存在于测试代码，
不构成公开协议面，第三方无需实现。

### 1.3 与 wire-protocol.md 第 3.2 节及 registry.toml 的核对结果

- 全表 16 行（§1.1 六行 + §1.2 十行）与 registry.toml 16 个公开协议 ID 逐条一致；
  与 wire-protocol.md §3.2 全表亦已对齐（本轮补登 /llm-share/proxy/1、
  /llm-share/offer/1 两行，并刷新 /dsh-acp/1、/repair/mcp/1、/llm-share/redeem/1
  状态列为已实现）。
- 差异 1：wire-protocol.md 第 9 节速查表写"内置协议 ID（5 个）、
  lib.rs:9-13"，现 proto_ids 实为 6 项（9-14 行）——文档滞后于代码，已在本文
  按 6 项登记（漂移登记，不改设计文档）。
- 差异 2：wire-protocol.md 速查表引 crates/p2p-mux/src/lib.rs:36 的
  MAX_STREAMS_PER_CONN=64，现常量位于 :73（值未变，行号漂移）。
- /p2p-base/identify/1 为全表唯一 impl=planned 条目（registry 对应 spec_status=draft，
  无规范页）；其余 15 个 ID 均 implemented 且已建/在建规范页（specs/，PROTO 轮交付）。

## 2. 业务协议 ID 命名与登记流程

- 命名：`/<组织或应用>/<协议名>/<纯数字版本>`，段字符集 [a-z0-9_-]，
  语法由 ProtocolId::new 机械校验（crates/p2p-protocol/src/lib.rs:36-62，
  规则见 wire-format.md 第 7 节）。示例：/myapp/chat/1。
- 登记 = 运行时注册：把实现 ProtocolHandler 接口的 handler 注册进节点
  HandlerRegistry（同文件 :122-140）即生效；无中心注册机构，无线上冲突面——
  协议 ID 只在本节点路由表内匹配。第三方应避免使用 /p2p-base/ 前缀以免
  与底座控制协议撞名。
- 收流分发：新流首帧解析出协议 ID 后查注册表；命中则把流交给 handler
  （已剥首帧），未命中关流上抛 UnsupportedProtocol。
- 并发与生命周期：handler 处理期间独占该流，返回即关流；同一节点可同时
  注册任意多个协议 ID，多协议并发即多条流并行。

## 3. 错误语义与故障排查

### 3.1 协议层错误（crates/p2p-protocol/src/lib.rs:70-84）

| 错误 | 确切行为 | 排查方向 |
|---|---|---|
| UnsupportedProtocol(id) | 收端注册表未命中：关流并上抛，发起方表现为流被立即关闭；无任何降级尝试 | 双方 ID 是否逐字符一致；handler 是否在该节点注册 |
| FrameTooLarge(n) | 写侧超 1 MiB 不写任何字节直接报错；读侧读出长度即断流，不收 payload | 改 chunked；核对 1 048 576 上限 |
| varint overflow | 长度前缀超 10 字节，或第 10 字节数据位 > 1/仍带继续位：报错断流，绝不静默回绕 | 对端编码器实现错误 |
| InvalidId(s) | ProtocolId 语法非法：注册或首帧解析时即失败 | 对照第 7 节正则 |
| MessageTooLarge(n) | chunked 重组超 64 MiB：报错断流 | 调小消息或分片传输 |
| Timeout(d) | request-response 全程一个超时，任一环节卡住即报 | 链路连通性、对端 handler 是否阻塞 |
| Io(e) | 长度与实际字节不符、流半途关闭等 | 查对端日志与网络 |

### 3.2 传输/身份层错误（crates/p2p-transport/src/lib.rs:77-105，noise.rs:175-193）

| 错误 | 确切行为 | 排查方向 |
|---|---|---|
| PeerMismatch{expected, actual} | 握手完成但推导 PeerId 与期望不符：立即终止连接 | 地址是否指向别的节点；PeerId 公式是否用对（对原始公钥取 SHA-256） |
| Dial{addr, reason} / DialChained | 拨号失败（不可达/拒绝/超时），source 链带内层原因 | 地址语法、端口放行、超时预算 |
| Handshake(s) / HandshakeChained | TLS/Noise 握手失败（含 ALPN 不一致、证书无身份扩展、验签失败） | ALPN p2p-base/1、证书扩展 OID、Noise 参数串 |
| IdentityUnverified | Noise 身份负载长度非 96 字节或验签失败：断链（crates/p2p-security/src/noise.rs:175-193） | 身份负载编码与域串 p2p-noise-xx-v1 |

### 3.3 中继控制面

状态机违规（首帧非 Reserve/Connect、重复预留等）回 Reject(code=4) 并断流；
错误码 1-7 全表见 node-lifecycle.md 第 2.4 节。未知新枚举变体在旧服务端
解析为空载荷，同样按 code=4 处理——失败可见，不会静默误解。

## 4. 版本演进与兼容策略

底座六条承诺（wire-protocol.md 第 8 节，全文引用）：

1. 帧封装无版本字段：语义完全由流首帧的协议 ID 决定，帧格式保持稳定。
2. 加字段不破坏：控制面 protobuf 只新增字段/变体，不改已有 tag 与类型；
   未知字段按 protobuf 语义跳过，旧实现可解析新消息。
3. 不兼容变更升协议 ID 版本号：/x/y/1 不兼容地变 /x/y/2，新旧 ID 在注册表
   并存路由，由发起方选择；禁止原地改既有 ID 语义。
4. 未知协议显式失败：关流上抛 UnsupportedProtocol，不猜测降级。
5. 控制面枚举加变体的失败可见性：新 kind 到旧服务端解析为空，按协议违规
   Reject(code=4) 断流。
6. 传输层演进走 ALPN（p2p-base/1）与 Noise 域串（p2p-noise-xx-v1）的版本段：
   破坏性变更升位后，版本不匹配在握手期即失败，不产生半兼容连接。

第三方协议作者的实施建议（与底座同构）：

- 信封式消息保留"类型头/版本字段 + 加法字段"空间；JSON 载荷未知字段必须忽略，
  protobuf 载荷天然前向兼容。
- 行为不兼容时发布 /yourapp/x/2，新旧 handler 并存一个过渡期；用"拨新 ID、
  收 UnsupportedProtocol 即回退旧 ID"实现探测，无需专门协商报文。
- 单帧装不下就 chunked（上限 64 MiB）；更大 payload 请自行在应用层再分块。
- 载荷内携带的对端身份仅供参考；权威身份永远以握手推导的流对端 PeerId 为准。
- 能力探活可直接复用 /p2p-base/ping/1（对端在线即应答）；协议级支持探测
  = 开流写你的协议 ID 看是否被关流。

## 5. 已知限制与规划中能力（明示，未实现）

| 能力 | 状态 |
|---|---|
| identify（公钥/地址交换协议） | 规划中，未实现（仅常量登记） |
| ACP 桥（/dsh-acp/1） | 已实现（apps/acp-agent；每连接会话上限 4 与 session-cap-reached 码为已登记未强制预留，见 specs/dsh-acp.md §8） |
| gossip pubsub（发布订阅） | 规划中，未实现（协调表 E5 候选登记，无任何代码） |
| metrics 观测 | 中继指标已实现（relay metrics + CLI），全栈 metrics 未实现 |
| QUIC/TCP 之外的新传输 | 规划外；ALPN/域串版本段为未来演进预留 |
