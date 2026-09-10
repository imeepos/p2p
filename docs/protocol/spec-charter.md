# p2p-base 协议规范章程（Spec Charter）

状态：v1 定稿（2026-09-10，PROTO 轮）。本文是 docs/protocol/ 规范体系的治理真值：
规范页模板、生命周期、规范性语言、符合性等级、注册表 schema、测试向量规范均以此为准。
章程自身修订视为契约变更，由项目负责人裁决。调研依据见
[../research/2026-09-10-protocol-standardization-survey.md](../research/2026-09-10-protocol-standardization-survey.md)。

## 1. 目的与读者

- 目的：任何第三方开发者**仅凭本目录文档**即可实现一个与本栈互通的节点或协议端点，
  不阅读本仓库源码。
- 读者：外部协议工程师、自有应用接入开发者、本仓库各波次实现者（规范页同时是
  实现的验收依据）。

## 2. 真值源分层（冲突裁决顺序）

| 层 | 位置 | 角色 |
|---|---|---|
| 行为权威 | crates/ apps/ 代码 | 事实标准；规范页与代码冲突时以代码为准并登记漂移 |
| 对外规范真值 | docs/protocol/specs/ | 逐协议自包含规范页（本章程 §7 模板）；对外唯一详细入口 |
| 机器可读索引 | docs/protocol/registry.toml | 全量公开协议 ID 清单；机械门禁数据源 |
| 内部实现对齐文档 | docs/design/wire-protocol.md | 按层组织的字节级事实文档；新增协议不再扩写 §8，§3.2 表保持全量 ID 索引 |
| 教程与导航 | docs/protocol/ 其余文件 | quickstart/wire-format/node-lifecycle 面向入门，不承载逐协议细节 |

裁决规则：代码 > 规范页 > 内部文档 > 教程。规范页不得复制实现源码，只写语义与字节。

## 3. 规范性语言

规范页使用 RFC 2119 风格关键词，中文规范页固定译法：

- **必须/不得（MUST/MUST NOT）**：违反即不兼容，互通必失败。
- **应当/不应当（SHOULD/SHOULD NOT）**：默认要求；偏离必须在实现文档中说明理由。
- **可以（MAY）**：可选行为。

"帧""流""协议 ID""PeerId"等术语沿用 docs/protocol/README.md §4 术语表，规范页不重定义。

## 4. 规范生命周期与变更流程

### 4.1 规范页状态（registry 的 spec_status 字段）

| 状态 | 含义 | 变更规则 |
|---|---|---|
| draft | 起草中；语义可调整 | 自由修订；常量已登记但无实现的协议（planned）页面恒为 draft |
| stable | 已对照实现复核，语义冻结 | 只能加法（新可选字段/新错误码）；修订需负责人复核合入 |
| frozen | 存量冻结（如 /im/chat/1 首版） | 不得改义；不兼容变更走 4.2 升版 |

### 4.2 兼容与升版（沿用底座六条承诺，wire-protocol.md §8）

- 加法不破坏：protobuf 只增字段/变体；JSON 载荷未知字段必须忽略。
- 不兼容变更 = 升协议 ID 版本号（`/x/y/1` → `/x/y/2`），新规范页新建，旧页顶部标注
  `Superseded by: <新 ID>` 但不删除；新旧协议 ID 并存路由一个过渡期。
- 禁止原地修改既有协议 ID 的语义。

### 4.3 变更流程

1. 提出任务卡（需求 + 验收，不含源码）→ 负责人裁决；
2. 实现/文档在 feature 分支交付，机械验收（含 protocol-registry 门禁）；
3. registry.toml、specs/ 页面、wire-protocol.md §3.2 三处同步为一次逻辑变更
   （registry 行为登记类改动，独立小提交）。

## 5. 符合性等级

| 等级 | 范围 | 规则 |
|---|---|---|
| Core | 互通必需：传输双路（QUIC 优先/TCP 兜底至少一路）、安全握手、帧封装、协议 ID 路由、所实现协议的规范页全部 MUST 项 | 不满足即不兼容 |
| Extended | 规范页标注"可选"的能力（如中继/打洞、chunked、特定协议可选帧） | 实现者可以不实现；不得向未声明支持的对端发送 Extended 帧 |

未实现 Extended 能力的节点收到相关帧时，按该协议错误语义显式失败（关流/回错误码），
不得静默忽略后继续按错误状态机推进。

## 6. 注册表 schema（registry.toml）

```toml
schema = 1
# 测试命名空间豁免：这些前缀的协议 ID 只存在于测试代码，不构成公开协议面
test_namespaces = ["/itest/", "/test/", "/p2p-lab/", "/myapp/", "/doc-example/"]

[[protocols]]
id = "/p2p-base/rendezvous/1"     # 公开协议 ID（逐字符）
family = "core"                   # core（底座控制面）| im | ecosystem
owner = "crates/p2p-discovery"    # 归属 crate/app 路径
impl = "implemented"              # implemented | planned（仅常量登记无实现）
spec_status = "stable"            # 见章程 §4.1（planned 实现的页面恒 draft）
spec = "specs/rendezvous.md"      # 规范页相对路径；planned 可省略
summary = "一句话职责"
vectors = ["rendezvous-register.json"]  # 相关向量文件名，可空
since = "2026-09-02"              # 首次进入公开协议面的日期
```

登记规则：新增公开协议 = 代码登记点 + registry 行 + spec 页（draft）三件套同轮；
机械门禁（scripts/check/protocol-registry.sh）双向核对代码字面量与 registry，
并核对 registry 每个 ID 在 wire-protocol.md 中有登记行。测试命名空间豁免清单
只增不减，新增豁免需负责人裁决。

## 7. 规范页模板（specs/*.md 固定骨架）

每页一个协议 ID（协议族可共页时按小节拆分，但每 ID 有独立锚点）。章节顺序固定：

```
# <协议 ID> 规范
状态：<draft|stable|frozen>；自 <日期>；归属 <owner>；符合性：<Core 项/Extended 项清单>
## 1. 概览            —— 解决什么问题、适用边界、与相邻协议的关系
## 2. 线格式          —— 帧布局（字节级）、类型头表、载荷字段表（含取值域与上限）
## 3. 时序与状态机    —— 开流顺序、事务时序、状态机与非法迁移、超时
## 4. 错误语义        —— 错误码/错误帧全表、每种错误的收发双方确切行为
## 5. 安全考量        —— 认证边界、信任假设、资源上限、滥用面与防护
## 6. 兼容与版本      —— 加法字段策略、探测方式、与旧版本互操作行为
## 7. 测试向量        —— 引用 docs/protocol/vectors/ 文件名（章程 §8 清单）
## 8. 实现状态与出处  —— 本仓实现位置（crate:文件）、已知偏差/漂移登记
```

硬性约束：单页 ≤300 行；必须包含全部章节（无内容写"无"并说明理由）；
MUST 语义 ≥1 处使用规范性关键词；字节常量（上限/超时/错误码）必须给出确切值；
不得出现实现源码；不复述底层帧封装（引用 wire-format.md），只写本协议的帧 payload 语义。

## 8. 测试向量规范（docs/protocol/vectors/）

- 单源双用：vectors/*.json 是唯一向量源。第三方按 vectors/README.md 的 schema 用任意
  语言消费；本仓 crates/p2p-conformance 读同一 JSON 做断言（对内防回归）。
- JSON 顶层统一形状：`{ "vector_set": "<名称>", "spec": "<依据文档>", "cases": [ {...} ] }`，
  case 内字段由各向量集在 vectors/README.md 自定义，但必须含 name 与 note。
- 首波向量集清单（文件名冻结，规范页 §7 按此引用）：

| 文件 | 覆盖语义 |
|---|---|
| varint.json | LEB128 边界值（0/127/128/16383/2^32-1/2^64-1）与溢出拒绝 |
| frame.json | 帧封装（长度前缀+payload）、1 MiB 上限、长度不符断流 |
| peer-id.json | 固定种子 → 公钥 → SHA-256 → base58 全链向量（≥3 组） |
| chunked.json | SINGLE/CHUNK/END 序列、重组上限、非法类型序拒绝 |
| rendezvous-register.json | SignedFields protobuf 序列化字节 + 固定种子 ed25519 签名 |
| relay-messages.json | RelayMsg oneof tag 1-9 黄金字节（含错误码表） |
| im-chat-envelope.json | 信封→ENVELOPE 帧、ACK 帧、MIME 白名单拒绝样例 |
| handshake-identity.json | Noise 身份负载 96 字节布局 + 域串验签；TLS 扩展 OID/ALPN 断言值 |

- 完整握手 transcript 向量（QUIC/Noise 全程）列 Extended 候选，后续轮评估。
- 消融要求：p2p-conformance 至少一组测试证明"翻转向量任一字节即红"。

## 9. 登记清单（本波执行落点）

- 底座控制面 5 页：specs/{identify,ping,rendezvous,relay,circuit}.md
- IM 族 5 页：specs/{im-chat,im-group,im-invite,im-ginvite,im-profile}.md
- 生态族 4 页：specs/{llm-share-proxy,llm-share-redeem,llm-share-offer,a2a}.md
- 桥族 2 页：specs/{dsh-acp,repair-mcp}.md
- 对齐件：builtin-and-versioning.md 全表刷新、wire-protocol.md §3.2 状态刷新与
  /llm-share/proxy|offer 两行补登、registry.toml 全量 16 ID。
