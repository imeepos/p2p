# ACP over P2P：单节点托管 agent，全网节点可视化操控 —— 设计方案 v0

> 状态: v1（已拍板，Q1-Q5 采默认建议，拍板记录见 §12） | 日期: 2026-09-04
> 依赖: [p2p-base-design.md](p2p-base-design.md)（底座分层）、[wire-protocol.md](wire-protocol.md)（协议 ID 与帧）、
> [gui-plan.md](gui-plan.md)（控制台宿主）
> 定位: 底座之上的应用层方案。不改通信内核，只新增应用协议与 apps/ 组件。
> 参照实现: deepseek-harness 仓库 `packages/acp/acp`（automation-only ACP 服务器）与
> `packages/subagent/subagent-acp`（out-of-process ACP 客户端，进程隔离哲学同源）。

把 DeepSeek Harness 的 ACP 自动化服务器（Agent Client Protocol，JSON-RPC over stdio）
嫁接到 p2p-base 底座之上：一个节点托管 agent，其他节点通过 P2P 连入后，用可视化控制台
与 agent 聊天、观察工具执行、批准权限、完成操作。

## 1. 背景与目标

### 1.1 两边的接口形状

| 侧 | 传输 | 分帧 | 关键语义 |
|---|---|---|---|
| ACP | stdio 字节流 | ndjson（按行） | 一连接多会话复用；连接断 → quiesce（取消/排空/flush）；会话持久化 + `session/list` / `session/resume` 跨重启恢复 |
| p2p-base | QUIC（quinn+TLS1.3 内嵌公钥）/ TCP（Noise XX + yamux） | varint 长度前缀，单帧 ≤ 1 MiB | 首帧协议 ID 分发；mDNS + rendezvous 发现；直连→打洞→加密中继降级链 |

结论：两边本质都是"字节流 + 分帧"，嫁接点天然存在；且 ACP 的"连接可死、会话不死"
（持久化 + resume）恰好对冲 P2P 网络的断线常态。

### 1.2 目标

1. 远程节点可通过可视化控制台与托管 agent 对话，实时看到思考、工具调用与结果。
2. 人类在环：工具权限以一次性批准按钮呈现给操作者或节点主人，超时即拒绝。
3. 断线重连不丢上下文：短期抖动无缝续连，长期中断走 ACP 原生 resume。
4. 标准 ACP wire 一个字节不改：任意标准 ACP 客户端/服务端保持互操作。
5. p2p-base 底座一层不侵：桥是底座的应用，不是底座的扩展。

### 1.3 非目标

- 不做人类交互级 UI 输出（diff 卡片、terminal 视图等 DSH 展示面）——ACP 本就是 automation-only。
- 不做 agent 间自动协商/额度托管（那是 E10 llm-share 的课题）；本文只做"人操控 agent"。

## 2. 立场与总架构

**一句话架构**：Rust 桥把一条 P2P 协议流变成一根"虚拟 stdio"，每个远程连接对应一个
专属 `dsh --profile acp` 子进程；GUI 作为标准 ACP 客户端，通过本地伴生进程接入
P2P 网络，把 ACP 语义更新直接渲染成可视化控制台。

**进程边界 = 连接边界**，与 ACP 的 quiesce 语义严丝合缝；这也是 harness 自家
out-of-process 子代理的同一哲学：隔离即设计。

```
        Agent 节点（家里/服务器）                          操作者节点（笔记本/手机）
┌─────────────────────────────────────────┐        ┌──────────────────────────────────┐
│  apps/acp-agent（Rust 常驻桥）            │        │  apps/gui（Web 控制台）            │
│  ├─ P2P Node（自研底座，QUIC/TCP+中继）    │        │  ├─ ACP 客户端（官方 TS SDK）       │
│  ├─ handler: /dsh-acp/1                  │        │  ├─ 聊天/工具时间线/权限按钮/选择器   │
│  │    每条入站流:                         │  QUIC  │  └─ 会话侧栏（连接级隔离）          │
│  │    ① 传输层已互认 PeerId → 查策略表     │  直连/  │                                  │
│  │    ② 握手帧（authz+scope+续连票据）     │  打洞/  │  apps/acp-console（Rust 伴生进程） │
│  │    ③ spawn 子进程（温池预热）           │ 加密中继│  ├─ 本地 WS（127.0.0.1+token）     │
│  │    ④ cwd 改写 → 工作区监狱             │◄──────►│  ├─ 纯字节泵 WS ⇄ P2P 流           │
│  │    ⑤ 剥离/白名单化 mcpServers          │ /dsh-acp│  └─ mDNS+rendezvous 发现          │
│  │    ⑥ ndjson ⇄ varint 帧互转            │  /1 流 │                                  │
│  ├─ 断线续连窗口 + update 环形缓存         │        │                                  │
│  ├─ 权限瀑布（静态策略→远程GUI→超时拒绝）    │        │  （GUI 永远是"某 peer 的控制台"，    │
│  └─ 退出阶梯: stdin EOF→宽限→SIGKILL      │        │    连自己节点=owner 全权 scope）    │
│                                          │        │                                  │
│  dsh --profile acp 子进程（每连接一个）     │        │                                  │
│   ├─ 会话持久化: list/resume/close        │        │                                  │
│   └─ API key 只在本进程环境                │        │                                  │
└─────────────────────────────────────────┘        └──────────────────────────────────┘
```

## 3. 组件清单（交付形态，非阶段）

| 组件 | 形态 | 职责 |
|---|---|---|
| `apps/acp-agent` | Rust bin | agent 侧端点：handler、authz、子进程监督、续连缓存、策略引擎 |
| `apps/acp-console` | Rust bin（可作 Tauri sidecar） | 操作者侧伴生：本地 WS ⇄ P2P 流哑泵 + 节点发现 |
| `apps/acp-common` | Rust lib | 两端共享：握手帧编解码、ndjson 分块重组、错误码 |
| `apps/gui` | Web | ACP 客户端 + 可视化控制台（协议智能全在这一层） |
| `p2p-cli` 扩展 | 子命令 | 节点主人策略管理：`acp allow/deny/list`（headless 管理面） |

分层纪律：三个 app 全部放 `apps/`，**不进 `crates/`**——底座 README 明文
"消息语义、存储、业务鉴权一律不在底座内"，桥是底座的应用。

## 4. 线协议：`/dsh-acp/1`

### 4.1 帧流动

首帧 = 协议 ID（底座标准开手）→ 1 个握手帧 → 之后全部是 ACP ndjson 字节块（双向、同一条流）。

```
客户端                                          Agent 桥
  │ 首帧: "/dsh-acp/1"                            │
  │ 握手帧: {"v":1,"conn":"<随机uuid>",           │
  │         "token?":"...",                      │
  │         "reattach?":"<断线前的conn uuid>"}     │
  │                                              │ ├─ PeerId ∈ 策略表? scope=?
  │                                              │ ├─ reattach 命中存活连接? → 走续连
  │ ◄── {"ready":{"scope":"workspace","agent":    │ ├─ 否则 spawn 子进程(或取温池)
  │        "home-agent","bridge":"1"}} ───────────│
  │ ══════════ 此后为纯 ACP ndjson 透传 ══════════ │
  │ {"jsonrpc":"2.0","id":1,"method":"initialize"}│ → 子进程 stdin
  │ ◄─ session/update 通知流(语义更新) ────────────│ ← 子进程 stdout
```

### 4.2 拍板项

1. **分块重组而非语义分帧**：ndjson 行任意长，桥按 ≤ 1 MiB 切帧、对端拼行；
   单行护栏 16 MiB（超限断流并留日志），杜绝内存炸弹。
   1 MiB 帧上限纯粹是底座约束，不构成 ACP 语义限制。
2. **握手帧是桥唯一的协议插入点**：只有一行、只出现一次，之后桥对字节完全透明。
   它解决三件事：authz 可观察拒绝（`{"denied":"peer-not-allowed"}`）、
   scope 通告、续连票据。
3. **顺序性免费**：QUIC/yamux 流可靠有序，ACP 的"每会话更新严格有序"天然满足；
   背压靠有界双向对拷（copy_bidirectional 语义），禁止无界缓冲。
4. **双版本独立演化**：`/dsh-acp/1` 是桥握手版本；ACP 版本由透传后的
   `initialize` 协商。协议 ID 登记进 `p2p-relay` 的 `proto_ids` 模块
   （底座唯一定义点），命名遵循 wire-protocol.md §3.1 语法。
5. **子进程纯净性**：桥 spawn 时接管子进程 stderr（落滚动日志文件），
   stdout 只走协议流；生产组合即 `pnpm dsh --profile acp` 同款。

## 5. 断线续连（核心体验决策）

**拍板：桥持进程 + 续连窗口 + 通知补发；窗口到期自动降级为"quiesce + resume"。**

```
流断 → 桥不杀子进程，进入续连窗口(默认 90s，可配)
     ├─ 子进程侧：stdio 仍由桥持有 → agent 不知道客户端走了 → in-flight turn 继续跑
     ├─ 更新侧：session/update 逐条进环形缓存(每会话 8 MiB 上限)
     ├─ 安全侧：outstanding 的 request_permission 一律立即代答 reject-once（无人值守=拒绝）
     │
窗口内同 PeerId 重连(握手帧带 reattach 票据，防跨设备劫持)：
     ├─ initialize 后，桥先补放缓存的 session/update（通知无 id，重放协议合法）
     ├─ in-flight 的 prompt 响应到达时按"旧连接迟到结算"投递（GUI 端约定，两端都是我们的代码）
     └─ UI 显示"已续连，补放 N 条错过的更新"
     │
窗口过期 / 子进程崩溃 → 退出阶梯：关 stdin(EOF=干净 quiesce→flush 持久化) → 宽限 → SIGKILL
     └─ GUI 走原生恢复：session/list → session/resume（上下文全在，只丢 in-flight 那轮）
```

诚实边界：**续连是桥约定（我们的 GUI 知道），resume 是协议标准（任何 ACP 客户端都会）**。
两条路都通，没有死角落。

## 6. 安全模型

远程驱动一个握着工具的 agent，这是生死线。

| 层 | 机制 | 拍板 |
|---|---|---|
| 认证 | 底座传输层已互认（QUIC TLS1.3 证书内嵌公钥 / Noise XX 握手即互认 PeerId） | 零成本拿到密码学身份，桥不另发明鉴权 |
| 授权 | 节点策略表：PeerId → scope | **默认拒绝**；owner 用 `p2p-cli acp allow <peer> --scope …` 显式授予（TOFU + 指纹确认） |
| 工作区 | cwd 改写 | scope=sandbox → `<root>/<peerId>/` 每 peer 监狱；scope=workspace → 锁定授权目录；owner 本机 = 全 root。**远程 peer 永远不能自指任意路径** |
| MCP | `session/new.mcpServers` = 远程任意命令执行 | **默认整字段剥离**；`allow_mcp` 白名单里 peer 只能**按名引用** node 配置预定义的服务定义（命令字节永远在 host 手里） |
| 工具 | ACP `request_permission` 瀑布 | ① 静态策略（read/think/fetch=allow，execute/edit/delete=ask）→ ② ask 路由到远程 GUI 内联按钮（60s 超时=reject）或按策略路由给 owner 本机 → ③ 一次性 grant，永不持久化（沿 harness 红线） |
| 凭据 | API key 只在子进程环境 | wire 上只有语义更新（ACP 原生设计）；中继为密文透传，端到端加密不落地 |
| 本地面 | GUI⇄console 本地 WS | 绑 127.0.0.1 + 随机 token（防浏览器 drive-by 打本地 WS） |

**必须点破的险情**：`session/new` 的 `mcpServers` 字段在原版 ACP 里是
"客户端让我执行什么就执行什么"——单机场景是特性，接到 P2P 上是 RCE。
上表 MCP 行是整个方案里最重要的一行。

## 7. 生命周期与故障矩阵

| 事件 | 桥行为 | GUI 看到 |
|---|---|---|
| 子进程启动失败 | 断流，错误入日志 | 连接失败提示 + 重试 |
| 子进程 mid-turn 崩溃 | 断流 | 错误气泡 + "尝试 resume"按钮（持久化前缀可恢复） |
| 网络抖动（窗口内） | 续连 | 续连横幅 + 补放更新 |
| 网络断（窗口过期） | EOF→quiesce→持久化 | 引导走 session/resume |
| 非授权 peer 连入 | 握手拒绝 + 审计日志 | （owner 侧日志可见） |
| 桥自身退出 | 全部子进程走退出阶梯 | 连接关闭；会话仍可 resume |
| 超限（单行>16MiB / 缓存>8MiB / 会话数>4） | 断流或拒绝 + 日志 | 明确错误，无静默 |

资源门禁拍板：每 peer 并发连接 = 1；每连接会话 ≤ 4（prompt 并发本来就是 ACP 的
每会话 1）；连接总数可配。容量参考：每连接一个 Node 进程约 150-300 MB，
8 GB 节点建议 ≤ 8 个并发控制台。温池预热 1-2 个**全新未服务**进程隐藏 ~1s 的
spawn 延迟——池化但不共享状态，隔离不破。

## 8. GUI 控制台：可视化控制 = ACP 语义的直接渲染

| ACP 面 | GUI 面 |
|---|---|
| `agent_message_chunk` | 流式聊天气泡 |
| `agent_thought_chunk` | 折叠"思考中"面板 |
| `tool_call` / `tool_call_update` | **工具时间线**：名称/状态/入参/结果——看见 agent 在干什么 |
| `request_permission` | 内联批准按钮 + 倒计时（超时自动拒绝并显示为已拒绝） |
| `config_option_update` | 模型 & reasoning effort 下拉（选项来自 agent 真实目录） |
| `usage_update` | 上下文占用条 |
| `stopReason` | 气泡结束态徽章 |
| `session/list` / `resume` / `close` | 会话侧栏（跨重连存活） |
| `initialize` 能力位 | 连接时显示真实能力（图片支持与否不撒谎） |
| 连接目录 | rendezvous 发现"提供 ACP 的节点" + mDNS 局域网零配置 + PeerId/二维码手动添加；显示 owner 声明名 + scope 徽章 |

统一视角：**GUI 永远是"某个 peer 的控制台"**——连别人节点是受限 scope，
连自己节点（loopback）是 owner 全权，同一套代码。

## 9. 明确不做（负空间即设计）

- 不改 harness 一行代码（`stream` 注缝留作测试，生产走桥）
- 不给 Rust 底座做 Node FFI（napi 路线把未完工的 S 阶段 facade 耦进 FFI，最脆路径）
- 不做协议级重设计（拆控制流/更新流/旁路流——失去标准 ACP 互操作）
- 不做进程池共享连接（跨 peer 会话可见性是安全漏洞）
- 不做 durable 权限、会话跨 peer 共享、凭据上 wire
- 桥不做 token 计费强制（agent 主人在 harness 侧配置，桥只管连接/会话门禁）

## 10. 设计取舍备忘（为什么是它）

1. **每连接一个子进程** vs 原生 transport：前者的进程边界恰好就是 ACP 的 quiesce
   边界，隔离免费、harness 零改动；个位数并发控制台容量毫无压力。正确胜过紧凑。
2. **续连约定收在桥内** vs 全面协议化：通知无 id 天然可重放，请求结算靠两端约定——
   15% 的复杂度换 85% 的体验收益，且降级路径是纯标准。
3. **协议智能全在 GUI**：桥是哑泵，Rust 侧永不解析 ACP 语义
   （除 cwd / mcpServers 两个安全改写点），协议升级不碰 Rust。

## 11. 依赖与风险

| 依赖/风险 | 说明 | 对策 |
|---|---|---|
| p2p facade S 阶段未装配 | `crates/p2p` Node 当前 `build()` 返回 `NotYetAssembled` | 桥实现挂在 facade 契约上，S 落地即可接线；期间以 p2p-itest 同款 link 接缝开发单件 |
| ACP TS SDK 来源 | GUI 需要 ACP 客户端库 | 优先用官方 npm SDK（与 harness 同款 `@agentclientprotocol/sdk`）；拿不到则手写 JSON-RPC 薄层（方法面仅 10 个，成本低） |
| harness SIGTERM 语义未知 | 退出阶梯依赖"stdin EOF = 干净 quiesce" | 阶梯以 EOF 为第一级（有 harness 代码背书），信号只作兜底；实现期验证 |
| 大图片消息 | prompt 内嵌 base64 可达数 MB | 分块重组 + 16 MiB 单行护栏已覆盖；持续超限属滥用，拒绝并留审计 |
| 首连引导 | 陌生节点如何建立信任 | TOFU + 指纹确认 + 二维码交换 PeerId；局域网 mDNS 可见但仍需 owner 授予 scope |

## 12. 待裁决问题（2026-09-04 ACP 波负责人已拍板：五项均采默认建议）

| # | 问题 | 默认建议 |
|---|---|---|
| Q1 | 续连窗口时长 | 90 s（可配）：过长占内存，过短退化为 resume |
| Q2 | 默认 scope | sandbox（每 peer 监狱）；workspace 授予必须显式 |
| Q3 | permission "ask" 的默认路由 | 远程操作者 GUI（交互场景自然）；owner 本机路由作为 per-peer 策略可选项 |
| Q4 | 协议 ID 命名 | `/dsh-acp/1`（备选：`/p2p-acp/console/1` 更中立） |
| Q5 | 未授权 peer 的拒绝可观察性 | 握手拒绝写审计日志即可，不向对方泄露策略细节 |

> 拍板记录（2026-09-04）：Q1=90s 可配；Q2=默认 sandbox；Q3=ask 默认路由远程 GUI；
> Q4=`/dsh-acp/1`（已登记 crates/p2p-relay/src/lib.rs `proto_ids`，wire-protocol.md §3.2 同步）；Q5=仅审计日志。
