# 远程电脑支持服务实施计划（P0b MVP）

> 状态: v1 | 日期: 2026-09-04 | 基线: [remote-support-design.md](remote-support-design.md) v1
> 定位: 设计到实现的落地拆解与契约裁决。动因看设计文档，本文只写"怎么拆、接缝在哪、怎么验"。
> 协调: RS 轨协调会话 session-5e733ff8；账本任务 T20-T28；与 E10（llm-share）轨并行，范围互斥。

## 1. P0b 范围（剪裁自设计 §10）

P1 项（fs_write+备份回滚、多 runner 路由、支付、反馈工单化、托盘 UI、收货评价页、断线续投）全部不在本计划。P0b 交付五件套：

1. repair-bridge 接入桥：stdio ⇄ p2p 流双向哑泵；
2. repair-helper 骨架：临时 MCP server 宿主（传输无关核心 + stdio 装配）；
3. 只读工具面：sys_snapshot / fs_read / fs_list / fs_search；
4. 本地执法核心：风险分级 / 红线 / scope 门 / 审批状态机（纯逻辑 crate）；
5. shell_exec 白名单闭集 + 执行记录 + 全链路 E2E + 单 runner 接入说明。

真机验收「3 类问题诊断+修复各 1 例」是人工里程碑，不入账本；代码里程碑以 T20-T28 机械验收（exit 0）为准。

## 2. 工件划分与归属

| 工件 | 形态 | 职责 | 任务 |
|---|---|---|---|
| crates/repair-bridge | bin: repair-bridge | stdio ⇄ p2p 流双向哑泵，不解析 MCP 语义 | T20 |
| crates/repair-helper | bin: repair-helper | p2p 端点 + MCP 宿主 + 工具面 + 执法接线 + 审计 + mint-ticket | T21/T23/T26 |
| crates/repair-enforce | lib | 分级/红线/审批状态机/shell 白名单语义（纯逻辑，无 IO） | T22/T24 |
| crates/repair-playbook | lib | playbook 结构化 markdown 解析与校验 | T25 |
| crates/p2p-itest/tests/repair_e2e.rs | itest | 桥 ⇄ 助手全链路 | T27 |
| docs/ops/repair-*.md | 文档 | runner 接入与真机演练 | T28 |

依赖向：T23←(T21,T22)；T24←(T22,T25)（白名单数据按 Q7 取三类 playbook 命令并集）；T26←T23；T27←(T20,T26)；T28←T27。T20/T21/T22/T25 互不依赖。

## 3. 契约裁决（冻结；改动须报 RS 协调）

### 3.1 隧道协议 ID

`/repair/mcp/1`——业务协议 ID（底座语法：段 repair、mcp，版本 1）。桥与助手间唯一流协议，
常量唯一定义点在 crates/repair-bridge，并登记 wire-protocol.md 协议表。首帧=协议 ID（沿底座
开流顺序），其后为底座帧设施封装的字节流，内容是透传的 MCP JSON-RPC。

### 3.2 MCP 传输与方法集

- 传输：MCP stdio 规范——行分隔 JSON-RPC 2.0（每行一个消息，空行忽略）。
- P0b 方法集：initialize、notifications/initialized、ping、tools/list、tools/call；未知方法按 JSON-RPC 规范返回 -32601。
- 版本协商：helper 支持集 {2025-06-18, 2025-03-26, 2024-11-05}，取与客户端请求交集的最新版；无交集返回 helper 支持的最新版，由客户端决定去留。
- tools/call 结果统一：文本内容 + truncated 标志（结构化输出 P1）。

### 3.3 ticket 契约

- 载荷 canonical JSON：{"ticket_id","helper_peer","bridge_peer","scope"("diag"|"fix"),"iat","exp"}，时间 Unix 秒。
- 编码：base64url(payload_json) + "." + base64url(ed25519 签名)；P0b 用测试密钥经参数注入，生产由调度签发属 P1。
- 校验（helper 侧全过才受理）：签名验真、exp 未过、scope 枚举合法、入流对端 PeerId==bridge_peer、工单存活表查重（一次性）；收货/超时即焚。
- 铸造：repair-helper mint-ticket 子命令（--key/--peer/--scope/--ttl），供开发与 itest。
- bridge 侧：ticket 不透明不校验，helper PeerId 由 --peer 显式指定。

### 3.4 执法语义（repair-enforce）

- 三档 read/write/danger；判定规则表数据化；发送侧打标、helper 侧独立重判，不一致以 helper 为准。
- 红线（无条件拒、无开关、不可配置绕过）：format/低级磁盘操作、触碰密码凭据文件、加密用户文件、批量删除（单调用多路径或递归删用户目录）、使杀毒软件失效。
- scope 门：diag 下 write/danger 直接拒；fix 下进入审批状态机。
- 审批：pending→approved/denied/timeout；60s 超时=拒绝，不可配置放行；通道为注入 trait（P0b 提供 CLI 行式实现，托盘 UI 属 P1）；时钟可注入便于测试。
- shell 白名单闭集：语义=argv[0] 白名单 + 参数模式校验，闭集外一律拒；清单数据 T24 按 Q7（卡慢/弹窗清理/C 盘空间三类 playbook 命令并集）填充。

### 3.5 工具面与门禁（P0b 只读集）

- 工具：sys_snapshot、fs_read、fs_list、fs_search（read 档）、shell_exec（按白名单重判）、session_report（read）。fs_write/fs_edit/backup_point 属 P1。
- 路径监狱：授权根参数化；全部路径参数 canonicalize 后必须落在授权根内，符号链接逃逸同拒。
- 输出门禁：单结果 ≤ 256 KiB，超限截断并置 truncated。

### 3.6 执行记录（P0b 交付物）

- helper 本地 JSONL 审计：每行一条调用记录（时间戳、clientInfo、tool、参数摘要、风险档、审批结果、结果摘要（截断）、耗时、错误）。
- session_report 导出结构化执行记录；工单页渲染与文件 diff 属 P1。
- 审计写失败必须留观测信号，禁止静默丢弃。

### 3.7 断线语义（P0b 简化）

- 桥：p2p 流断或 stdin EOF→排空在途帧后非零退出 + stderr 留因；不做自动重连。
- helper：桥断即流关闭；挂起中的审批视同拒绝（沿设计 §5 拍板 4）。

## 4. 任务表（账本 T20-T28）

| 任务 | 分支 | 范围 | 验收（exit 0） | 依赖 |
|---|---|---|---|---|
| T20 接入桥 | feat/rs-bridge | crates/repair-bridge/** + wire-protocol.md 登记 + 根 Cargo.toml 仅追加 | cargo test -p repair-bridge && make check | — |
| T21 MCP 宿主 | feat/rs-helper-host | crates/repair-helper/**（宿主框架，注册表空）+ 根 Cargo.toml 仅追加 | cargo test -p repair-helper && make check | — |
| T22 执法核心 | feat/rs-enforce-core | crates/repair-enforce/** + 根 Cargo.toml 仅追加 | cargo test -p repair-enforce && make check | — |
| T23 只读工具面 | feat/rs-tools-readonly | repair-helper 工具模块（4 只读工具+监狱+门禁） | cargo test -p repair-helper && make check | T21,T22 |
| T24 shell 白名单 | feat/rs-shell-whitelist | repair-enforce 白名单数据+shell 判定 | cargo test -p repair-enforce && make check | T22,T25 |
| T25 playbook 格式 | feat/rs-playbook | crates/repair-playbook/** + docs/playbooks/ 三类草案 | cargo test -p repair-playbook && make check | — |
| T26 helper p2p+票据+审计 | feat/rs-helper-p2p | repair-helper p2p 接入/ticket 校验/mint-ticket/JSONL 审计 | cargo test -p repair-helper && make check | T23 |
| T27 全链路 E2E | feat/rs-e2e | crates/p2p-itest/tests/repair_e2e.rs | cargo test -p p2p-itest --test repair_e2e && make check | T20,T26 |
| T28 runner 接入文档 | docs/rs-runner-integration | docs/ops/repair-runner-integration.md + repair-p0b-drill.md | 两文档在位 + make check | T27 |

## 5. 批次与节奏

- 批次一（并行）：T20/T21/T22——三个新 crate 零交集。
- 批次二（并行）：T23/T25——两个不同工件零交集。
- 批次三（部分并行）：T24 ∥ T26 → T27 → T28。
- 合并纪律：批次内完成即合并不等齐；根 Cargo.toml/Cargo.lock 冲突一律 feature 侧消化；收尾按 coordination.md 规则 5（rebase main 反向同步 → 主树核对后 ff-only → push origin → 清 worktree/分支）。
- E10 轨（T17/T18 llm-share）与本轨互不动对方 crate 与账本条目；根 Cargo.toml 双方都只追加。

## 6. 已知留白（P1 前置登记）

托盘 UI 与审批可视化（P0b 用 CLI 行式审批替代）；断线续跑缓存与补投（acp §5 机制移植）；fs_write+备份/diff/回滚与修改记录双写；多 runner 路由、调度签发正式 ticket、支付与工单页；screen 工具与会员制（P2）。
