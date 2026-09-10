# /dsh-acp/1 规范

状态：stable；自 2026-09-04；归属 apps/acp-common；符合性：Core = 握手帧、ndjson 分块
透传、错误码、一连接一子进程宿主语义；Extended = 断线续连（票据/环形缓存补放）、
分享令牌兑换准入。

## 1. 概览

本协议把一条 p2p-base 逻辑流变成一根"虚拟 stdio"：操作者节点（客户端）经 P2P 网络
接入 agent 节点（桥）上托管的 ACP（Agent Client Protocol，JSON-RPC over ndjson/stdio）
子进程。桥协议只定义桥自身插入的语义：一个握手帧、之后的纯 ndjson 字节透传、断线续连
约定与两个安全改写点（mcpServers 处置、request_permission 权限瀑布）。

适用边界与相邻协议：

- ACP 语义本身不属于本协议：握手完成后字节对桥透明，ACP 版本由透传的 `initialize`
  请求协商（双版本独立演化，见 §6）。
- 帧封装（varint 长度前缀、单帧 1 MiB 上限）复用底座，见 docs/protocol/wire-format.md；
  本文只写帧序列与载荷语义。
- 宿主语义：每条入站连接必须对应一个专属 agent 子进程（默认命令
  `pnpm dsh --profile acp`），进程边界 = 连接边界 = ACP quiesce 边界；跨连接共享
  子进程必须禁止（跨 peer 会话可见性即安全漏洞）。
- 身份来源：流对端 PeerId 由底座安全握手互认并随流下传；任何载荷内自报身份仅作
  参考，不得作为授权依据（见 §5.1）。

## 2. 线格式

流上字节按底座帧封装。帧序列固定为三段：

| 阶段 | 内容 | 方向 |
|---|---|---|
| 开手 | 首帧 = 协议 ID `/dsh-acp/1` 的 UTF-8 字节（底座标准） | 客户端→桥 |
| 握手 | 恰一行 ndjson JSON（各一行，见 §2.1） | 双向各一行 |
| 数据 | 纯 ndjson 字节块，双向同流，直至断流 | 双向 |

### 2.1 握手帧

客户端握手行（一行 JSON，UTF-8）字段：

| 字段 | 类型 | 取值域 | 语义 |
|---|---|---|---|
| v | number | 必须 = 1 | 握手版本；其他值拒绝 |
| conn | string | UUID | 客户端生成的连接标识；日志与子进程 stderr 日志命名用 |
| token | string | 可选 | 分享令牌：策略表未命中时凭 token 兑换准入（§5.1） |
| reattach | string | 可选，UUID | 续连票据：携票据走续连路径（§3.3） |

未知字段必须拒绝（握手行是信任边界，fail-fast）；缺 v/conn、UUID 非法或类型不符
同样拒绝。实现不得容忍未知字段——容忍会把协议漂移推迟成运行期错位。

桥回执行是且仅是下列二者之一（untagged 二选一）：

- `{"ready":{"scope":"sandbox|workspace|owner","agent":"<名>","bridge":"1",
  "ticket?":"<uuid>"}}`
- `{"denied":"<错误码>"}`，错误码取值见 §4。

ready 字段语义：scope = 本连接生效的工作区边界；agent = agent 声明名；bridge = 桥
协议版本，当前 `"1"`；ticket = 续连票据，加法字段（缺省不序列化），仅 fresh 路径
签发、reattach 路径回带原票据。客户端必须容忍缺省 ticket（旧桥兼容）。

### 2.2 ndjson 分块与护栏

数据相字节完全透明，桥必须保持字节序与完整性；行界还原规则两端对称：

- 发送侧：把一行（不含行尾换行）按 ≤ 1 048 576 字节切帧，行尾 `\n` 以独立 1 字节
  帧收尾；被改写的行（§5.2）重序列化后自带行尾换行。
- 接收侧：逐帧拼接直到出现 `\n`；`\r\n` 必须归一为 `\n`；空行原样透传。
- 单行护栏 16 777 216 字节（16 MiB，含行尾换行）：累积超限必须立即断流并留日志，
  不得缓冲无界增长（内存炸弹防线）。
- 半行终止（流结束仍有未换行字节）必须显式报错（ndjson-truncated），不得静默丢弃。

## 3. 时序与状态机

### 3.1 连接建立

桥侧顺序固定，任一步失败走 §4 拒绝路径：

```
客户端                                          桥
 1. 开流，首帧协议 ID                →   2. 流身份归属：取握手互认 PeerId；
                                            无归属（裸流）= fail-closed 拒绝
 3. 握手行（10 秒内，超时即拒）      →   4. 授权：策略表 PeerId→scope；
                                            未命中凭 token 兑换；过 authz 闸
 5. ←────────────────────────────────   5'. 资源门禁：每 peer 并发 1、
                                            连接总数可配（默认 8）
 6. ready（fresh 带 ticket）或 denied ←
 7. initialize 起 ACP 数据相         ⇄   8. fresh：顶替遗留槽位→cwd 监狱→
                                            spawn 子进程→ready；续连：§3.3
```

握手读超时 10 秒；超时、读失败或非法行一律按 handshake-malformed 拒绝。

### 3.2 数据相宿主语义

- 桥为每连接 spawn 一个专属子进程（温池可预热全新进程隐藏 spawn 延迟，但池化不得
  共享状态）；子进程 stderr 必须被桥接管落滚动日志，stdout/stdin 只走协议流。
- 客户端→子进程：帧重组为行原样直写 stdin（除 §5.2 改写点外字节零改动）。
- 子进程→客户端：逐行读 stdout 分块回写；输出面必须单写者，通道序即 wire 序。
- 背压：有界双向对拷，任何方向不得缓冲无界增长。

### 3.3 断线续连（桥约定，Extended）

状态机：`attached ⇄ detached(续连窗口) → exited`。

- 客户端断流：桥不得杀子进程，进入续连窗口（默认 90 秒，可配，下限 1 秒）；
  子进程 stdin 仍由桥持有，in-flight turn 继续运行。
- 窗口内缓存：method 恰为 `session/update` 的子进程行（会话键 = params.sessionId）
  逐行入每会话环形缓存，每会话队列字节总量必须 ≤ 8 388 608（8 MiB），超限丢最旧
  并留日志；其他子进程行（如 prompt 响应）不缓存、丢弃并留日志（迟到结算由两端
  上层约定，不属本协议）。
- 窗口内 outstanding 的 request_permission 必须立即代答 reject-once（无人值守 =
  拒绝）。
- 窗口内重连：握手行 reattach = 票据；桥校验票据存在且绑定同一 PeerId（跨设备持
  他人票据必须拒绝）。接管后 ready 回带原票据；客户端发出 initialize（该请求过桥
  写入子进程）后，桥必须先向 wire 补放一行桥约定通知
  `{"jsonrpc":"2.0","method":"dsh/bridge/reattach","params":{"replayed":N}}`
  （无 id 通知，N = 补放条数），再按会话名序、行序补放缓存行，然后恢复实时透传。
- 窗口内不带票据的新连接视为放弃续连：桥必须顶替（supersede）遗留槽位（Shutdown，
  走退出阶梯）后按 fresh 流程新建。
- 窗口过期 / 子进程崩溃 / 桥停机：退出阶梯必须为 关 stdin（EOF = 干净 quiesce，
  子进程 flush 持久化）→ 宽限等待（默认 10 秒，可配，下限 1 秒）→ SIGKILL。
  之后客户端走 ACP 原生恢复：session/list → session/resume（上下文全在，仅丢
  in-flight 一轮）。

续连是桥约定（两端必须同时实现）；resume 是 ACP 标准行为（任何 ACP 客户端可用）。
两条恢复路径并存，无死角落。

## 4. 错误语义

握手期错误必须回一行 denied 再关流并审计；错误码只回词法码，不得向对端泄露策略
细节。数据相护栏违例必须立即断流。拒绝与超限动作必须留审计或日志信号，禁止静默。

| 错误码 | 触发 | 行为 |
|---|---|---|
| peer-not-allowed | 流无身份归属；策略表未命中且无可兑换 token；authz 闸拒绝 | denied + 关流 + 审计 |
| handshake-malformed | 握手超时/读失败/JSON 非法/未知字段/v 不为 1/UUID 非法 | denied + 关流 |
| conn-cap-reached | 每 peer 并发连接超 1，或连接总数超上限 | denied + 关流 + 审计 |
| session-cap-reached | 每连接会话数超上限（已登记，预留，见 §8） | denied + 关流 + 审计 |
| cwd-denied | scope=workspace 未配置工作区/工作区未知；监狱越界 | denied + 关流 + 审计 |
| subprocess-failed | 子进程 spawn 失败 | denied + 关流 + 审计 |
| reattach-ticket-invalid | 票据不存在/槽位已退出/绑定 PeerId 不符 | denied + 关流 + 审计 |
| line-too-long | 单行超 16 777 216 字节 | 立即断流 + 日志 |
| frame-too-large | 单帧超 1 048 576 字节 | 立即断流 + 日志 |
| ndjson-truncated | 流以半行收尾 | 断流报错 + 日志 |
| share-reuse-denied / share-expired / share-revoked / share-exhausted | token 兑换被分享台账拒绝（重复绑定/过期/撤销/激活额满） | denied + 关流 + 审计 |

mcpServers 违例（§5.2）不关流：可应答请求回 JSON-RPC 错误 `{"code":-32602,
"message":"mcp-servers-rejected"}`（带原 id）；notification（无 id）静默丢弃仅审计。
未注册本协议 ID 的对端按底座语义关流上抛 UnsupportedProtocol，不产生本协议层交互。

## 5. 安全考量

### 5.1 认证、授权与准入

- 认证由底座承担：QUIC TLS1.3 证书内嵌公钥 / Noise XX 握手即互认 PeerId（见
  docs/design/wire-protocol.md §5/§6）；桥必须以流下传的互认 PeerId 为授权查询键，
  不另发明鉴权。
- 授权默认拒绝：策略表条目 PeerId → {scope, allow_mcp, ask_route, workspace,
  fingerprint, granted_at}，查无即拒；授予由节点主人显式操作（TOFU + 指纹确认）。
  策略表文件存取必须原子（临时文件 + rename），文件损坏必须显式报错，禁止静默
  回退空表——默认拒绝不等于吞存储故障。
- 兑换准入（可选面）：策略表未命中且握手带非空 token 时，凭分享台账兑换激活
  （激活即写入策略表）；兑换失败按 §4 share-* 码拒绝；台账存储故障一律按拒
  （fail-closed）。
- 非 owner 授权还必须通过 authz 闸（check(peer, acp.session)），绑定缺失即拒；
  owner 不进 authz，由路由侧 scope 分流。

### 5.2 两个安全改写点（桥仅有的协议插入语义）

1. mcpServers 处置（session/new）：原版 ACP 中该字段等于"客户端让我执行什么就执行
   什么"，接到 P2P 即远程任意命令执行。默认（该 peer 白名单为空）必须整字段剥离后
   转发；白名单 peer 只能按名引用（每项 `{"name":"<名>"}`），且名字必须同时在
   allow_mcp 白名单与 host 预定义服务定义表中，桥把数组整体替换为 host 定义——
   命令字节永远在 host 手里。违例（白名单外名字、非按名引用、host 未预定义、
   字段非数组）必须整请求拒绝（§4），不转发子进程。剥离/替换/拒绝动作必须留审计。
2. request_permission 权限瀑布：子进程行 method 以 request_permission 结尾且携带
   可应答 id（非 null）时拦截。静态策略先行：kind=think 桥直接代答选中第一个
   kind 以 allow 开头的选项（该请求不透传客户端）；kind=read/fetch 仅 owner 本机
   （loopback）场景可静态放行，远程驱动一律进 ask（保守默认）。ask 路由：
   remote_gui（默认）透传客户端并登记 outstanding，60 秒（可配，下限 1 秒）未应答
   桥代答 reject-once（`{"outcome":{"outcome":"cancelled"}}`）；owner_local 不透传，
   本地审计并立即 reject-once 占位。ask 路径还必须前置 authz 闸
   check(peer, acp.execute)，拒绝则本地直拒不弹窗。grant 必须一次性，桥不得
   持久化任何许可状态（allow_always 选项不作桥侧记忆）。

### 5.3 工作区监狱与凭据

- cwd 按 scope 解析，远程 peer 永远不能自指任意路径：sandbox = `<sandbox_root>/
  <净化(peerId)>/`（每 peer 独立目录，不存在则创建；PeerId 必须经字符白名单净化
  防路径段注入，目录必须组件级前缀校验不逃逸）；workspace = 锁定配置目录
  （symlink 必须解析到真实目标后锁定；未配置即拒）；owner = 继承桥 cwd（全 root），
  仅限本机 loopback 场景，远程授予 owner 等同交出整机，属操作者责任（TOFU 面
  拦截）。
- 凭据（API key）只进子进程环境；wire 上只有语义更新；中继转发为密文透传，
  端到端加密不落地。
- 资源上限汇总：单帧 1 048 576 / 单行 16 777 216 / 缓存 8 388 608 / 握手 10 秒 /
  续连窗口 90 秒 / 权限应答 60 秒 / 退出宽限 10 秒 / 每 peer 连接 1 / 连接总数
  默认 8；全部超限必须显式失败并留可观测信号，禁止静默。

## 6. 兼容与版本

- 握手 v=1 与 ready.bridge="1" 是桥协议版本；ACP 版本由透传后的 initialize 协商，
  双版本独立演化。桥实现不得解析透传字节的 ACP 语义（§5.2 两个改写点除外）。
- 加法策略：ClientHello.token / ClientHello.reattach / Ready.ticket 均为"缺省不
  序列化"的加法字段，旧端解析新帧不受影响；数据相对 ACP 语义零假设，ACP 自身
  升级不需要变更本协议。
- 握手行未知字段拒绝是 v1 明确语义；未来扩展握手必须升协议 ID 版本（新建规范页），
  不得在 /dsh-acp/1 原地改义。
- 探测：开流写本协议 ID，未注册对端关流（UnsupportedProtocol）即不支持。
- 续连降级：不支持续连约定的客户端断线后直接走 ACP 原生 session/list →
  session/resume，语义完整。

## 7. 测试向量

无。首波向量集清单（spec-charter §8）未含本协议：握手与护栏语义由 apps/acp-common
单元测试、apps/acp-agent 回环套件（stub/真子进程双模式）与 crates/acp-pump 回环
传输套件覆盖；握手帧黄金行（ClientHello/Ready/denied）与 16 MiB 护栏边界向量列为
后续向量集候选。

## 8. 实现状态与出处

| 语义 | 实现位置 |
|---|---|
| 握手帧编解码、分块重组、错误码、策略表、分享台账 | apps/acp-common/src/{handshake,chunk,error,policy,share}.rs |
| 流编排（归属→授权→门禁→分流）与拒绝路径 | apps/acp-agent/src/session.rs、handler.rs |
| fresh/reattach 连接路径、wire 有界双向泵 | apps/acp-agent/src/conn.rs、pump.rs |
| 槽位簿记、子进程 spawn/stderr 接管、退出阶梯 | apps/acp-agent/src/child.rs、subprocess.rs、router/ |
| 续连环形缓存、wire 行谓词、补放宣告 | apps/acp-agent/src/reattach.rs |
| mcpServers 改写、权限瀑布、cwd 监狱 | apps/acp-agent/src/{mcp,permission,jail}.rs |
| 操作者侧泵（本地 WS ⇄ P2P 流哑泵） | crates/acp-pump/src/ |

已知偏差与漂移登记：

- acp-over-p2p-design.md §4.1 示例 ready 帧无 ticket 字段：以 apps/acp-agent/README.md
  桥约定 v1 为准（ticket 为加法字段），设计文档示例视为起草期快照。
- 设计文档 §7 拍板"每连接会话 ≤ 4"：上限常量与 session-cap-reached 错误码已登记，
  当前实现未强制执行（会话并发由 ACP 自身每会话单 prompt 约束）；本页 §4 按预留
  登记。属文档拍板先行、实现未跟的既知间隙。
- 除上述两条外无已知漂移；规范页与实现冲突时以代码为准并登记本节。
