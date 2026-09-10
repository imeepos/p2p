# 协议规范化调研（2026-09-10，PROTO 轮前置调研）

状态：定稿。调研人：项目负责人（协调会话）。结论供 `docs/protocol/spec-charter.md`
（规范章程）与本轮六张任务卡引用；事实基线为 2026-09-10 main（63842f0）。

## 1. 代码事实：公开协议 ID 全量清单

机械提取：对 crates/ apps/ 全部 .rs 做协议 ID 形态字符串扫描（`/段/段/数字`），
剔除测试命名空间（/itest/、/test/、/p2p-lab/、/myapp/、/repair/unknown/）后共 16 个公开 ID：

| # | 协议 ID | 定义点（事实） | 家族 |
|---|---|---|---|
| 1 | /p2p-base/identify/1 | crates/p2p-relay/src/lib.rs proto_ids（仅常量，无 handler） | 底座控制面 |
| 2 | /p2p-base/ping/1 | 同上 + crates/p2p-swarm/src/swarm/ping.rs（已实现） | 底座控制面 |
| 3 | /p2p-base/rendezvous/1 | 同上 + crates/p2p-discovery/src/rendezvous/ | 底座控制面 |
| 4 | /p2p-base/relay/1 | 同上 + crates/p2p-relay/src/control.rs | 底座控制面 |
| 5 | /p2p-base/circuit/1 | 同上 + crates/p2p-relay/src/circuit.rs | 底座控制面 |
| 6 | /im/chat/1 | crates/p2p-chat/src/wire.rs | IM 族 |
| 7 | /im/group/1 | crates/p2p-chat/src/group_wire.rs | IM 族 |
| 8 | /im/invite/1 | crates/p2p-chat/src/wire_invite.rs | IM 族 |
| 9 | /im/ginvite/1 | crates/p2p-chat/src/ginvite_wire.rs | IM 族 |
| 10 | /im/profile/1 | crates/p2p-chat/src/profile_wire.rs | IM 族 |
| 11 | /llm-share/proxy/1 | crates/llm-share-proxy/src/wire.rs | 生态族 |
| 12 | /llm-share/redeem/1 | crates/llm-share-link | 生态族 |
| 13 | /llm-share/offer/1 | crates/llm-share-offer | 生态族 |
| 14 | /a2a/1 | crates/a2a/src/lib.rs:18 PROTOCOL_ID | 生态族 |
| 15 | /dsh-acp/1 | apps/acp-common/src/consts.rs:5 | 生态族 |
| 16 | /repair/mcp/1 | crates/repair-bridge/src/lib.rs | 生态族 |

## 2. 文档现状与缺口（对照 2026-09-06 DOC 波交付）

已有：docs/protocol/ 五文件（README/quickstart/wire-format/node-lifecycle/builtin-and-versioning，
806 行）+ docs/design/wire-protocol.md（字节级 v1，含 §8.1-8.5 业务登记）。缺口五条：

1. 逐协议 RFC 式规范页缺失：wire-protocol.md 是"内部实现对齐文档"（按层组织、按出处标注），
   不是自包含的逐协议规范；第三方实现 /llm-share/proxy/1 时无成文规范可依。
2. 生态族线格式未成文：/a2a/1、/dsh-acp/1、/llm-share/proxy|redeem|offer/1、/repair/mcp/1
   只在 §3.2 表中有一行摘要，帧格式/状态机/错误语义散在各 design 文档与源码注释。
3. 注册表滞后：builtin-and-versioning.md §1.2 缺 /a2a/1、/im/ginvite/1、/im/profile/1；
   /dsh-acp/1 标"桥实现未落"（实际 acp-agent 已落地）；/llm-share/redeem/1 标"随波落地"（已落地）。
4. 无机器可读注册表与防漂移门禁：代码与文档的一致性靠人工核对（DOC 波抽检 8/8 为人工动作）。
5. 无第三方可用的测试向量/符合性套件：varint/帧/PeerId/签名等关键语义无黄金向量，
   独立实现者只能对着 Rust 源码猜边界。

## 3. 外部生态参照（2026-09-10 web 调研）

- libp2p specs（github.com/libp2p/specs）：独立规范仓库，"规范框架（Spec Lifecycle +
  Document Header）+ 核心抽象（peer-ids/addressing/connections）+ 逐协议规范（ping/identify/
  rendezvous/relay/DCUtR/noise/tls/quic…）+ 索引"四层结构；规范仓库同时是演进的协调点。
- Nostr NIPs：编号规范文档 + 必选/可选分级 + 简单提案流程，低门槛带来生态扩散。
- Matrix / AT Protocol：版本化房间/词表 schema + 一致性测试（Dendrite Complement /
  interop test files）支撑多实现互通。
- 共性结论：生态接入能力 = 自包含规范页 + 机器可读注册表 + 黄金测试向量 + 明确的
  版本与兼容承诺。四者齐备，第三方才不需要读参考实现源码。

## 4. 设计裁决（D1-D8，负责人定稿）

- D1 不改线协议：v1 线协议已冻结并经真实链路验证；本轮只做规范化（规范页/注册表/向量/
  门禁），发现的漂移只登记不改代码；协议行为变更另走契约流程。
- D2 真值源分层：代码 = 行为权威；docs/protocol/specs/ = 对外规范真值（逐协议自包含）；
  docs/design/wire-protocol.md = 内部实现对齐文档（新增协议不再扩写 §8，§3.2 表保持全量
  ID 索引）；docs/protocol/registry.toml = 机器可读索引（门禁数据源）。
- D3 规范章程先行：模板/生命周期/MUST 语言/符合性等级由章程冻结（负责人合入 main），
  六张卡在冻结模板上并行，避免返工。
- D4 注册表机械门禁：scripts/check/protocol-registry.sh 双向核对（代码字面量⇄registry、
  registry⇄wire-protocol.md 存在性），挂接 make check，红绿自测入 gate-tests 风格验收。
- D5 测试向量单源双用：docs/protocol/vectors/*.json 为唯一向量源（语言中立、第三方可直接
  消费），新建 crates/p2p-conformance 读同一 JSON 断言（对内防回归，消融可证）。
- D6 向量范围（首波 8 组）：varint、frame、peer-id、chunked、rendezvous-register、
  relay-messages、im-chat-envelope、handshake-identity（身份负载布局+验签向量；
  完整握手 transcript 向量列 Extended 候选，后续轮评估）。
- D7 任务拆分零文件交集：PT1 门禁（scripts/Makefile）、PT2 向量库（新 crate+vectors/）、
  PT3 控制面规范页（5 新文件）、PT4 IM 族规范页（5 新文件）、PT5 生态族规范页（4 新文件）、
  PT6 内置表对齐+桥族规范页（2 新文件 + builtin-and-versioning.md + wire-protocol.md）。
- D8 治理归位：规范页生命周期（draft→stable→frozen）与变更流程入章程；规范变更视为
  契约变更，经负责人裁决；本波交付页面合入即 stable（负责人逐页复核）。

## 5. 风险与边界

- prost 手写 derive（无 protoc）：relay-messages 向量以既有实现字节为基线，向量同时冻结
  该编码（防手写 drift），登记于章程向量清单。
- /llm-share/offer/1 的线上语义需 PT5 读源码确证（capability 声明发布通道的帧形态），
  如与 builtin 文档描述不符以代码为准并登记漂移。
- 门禁豁免面：测试命名空间（/itest/、/test/、/p2p-lab/、/myapp/、/doc-example/）不构成
  公开协议面，registry 声明豁免；新增豁免需负责人裁决。
