# 文档索引

本目录是 p2p-base 底座全部文档的入口。按用途分四类：

| 分类 | 目录 | 收录内容 |
|---|---|---|
| design | `docs/design/` | 设计方案与规范：总方案、字节级线协议规范 |
| ops | `docs/ops/` | 实验环境、多机部署与运维方案 |
| research | `docs/research/` | 外部调研笔记（法律法规等），不构成结论性意见 |
| notes | `docs/notes/` | 复盘/评审/裁决记录（预留目录；实验问题复盘脱敏后落此处，见 ops 文档第 6 节） |

## 文档清单

| 文档 | 简介 |
|---|---|
| [coordination.md](coordination.md) | 并行开发协调表：包/分支/负责人/范围/状态/验收与并行规则；协调者维护，各开发会话只读 |
| [design/p2p-base-design.md](design/p2p-base-design.md) | 总设计方案 v0：需求决策、分层架构、业务 API 表面、发现/穿透/中继降级链、分期路线与风险 |
| [design/wire-protocol.md](design/wire-protocol.md) | 字节级线协议规范 v1：帧格式、协议 ID 语法与内置全表、开流顺序、握手流程、PeerId 推导、版本演进策略；全部常量与 crates/ 代码对齐并标注出处 |
| [design/idle-token-sharing-plan.md](design/idle-token-sharing-plan.md) | 闲置 LLM 额度共享网络方案（barter-first）v0：双层额度模型、预授权-结算两阶段、llm-share 三协议 ID、账本/收据/封号责任机制、合规红线与分阶段路线 |
| [design/acp-over-p2p-design.md](design/acp-over-p2p-design.md) | ACP over P2P 方案 v0（待审核）：/dsh-acp/1 桥协议、每连接一个 agent 子进程、断线续连+resume 双路径、PeerId 策略表与工作区监狱、GUI 可视化控制 |
| [ops/experiment-env.md](ops/experiment-env.md) | 实验环境与多机验证方案：四机拓扑（3 局域网 + 1 公网引导）、E0-E4 阶段计划、验收清单、凭据安全红线 |
| [research/p2p-legal-risk-cn.md](research/p2p-legal-risk-cn.md) | P2P 应用国内法律法规风险调研：金融类 P2P 为禁区、技术型 P2P 的电信资质/内容/版权合规重心 |
| [design/remote-support-design.md](design/remote-support-design.md) | 远程电脑支持服务（AI 维修坐席）方案 v1：任意 agent 接入（临时 MCP server + 接入桥）、工具面与双端执法、执行记录与文件修改交付、0-6 工单闭环与分期 |

## 约定

- 设计文档描述动机与目标（"为什么/要什么"）；wire-protocol.md 描述线上现实（"字节长什么样"）；
  两者冲突时以 crates/ 代码为准，并回改文档。
- 各文档头部标注状态与日期；进度类信息（谁合并了什么）以 `coordination.md` 变更记录为准。
