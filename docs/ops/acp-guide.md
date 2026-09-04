# ACP 运维手册（节点主人）

读者：要把自己的 agent 节点接入 P2P 网络、给远程操作者开放可视化操控的节点主人。
设计依据：[acp-over-p2p-design.md](../design/acp-over-p2p-design.md)（§6 安全模型、
§7 生命周期与故障矩阵、§12 拍板记录）；桥级私有约定（续连票据、补放格式、
mcpServers 处置）见 [apps/acp-agent/README.md](../../apps/acp-agent/README.md)。

## 1. 部署形态

agent 节点常驻 acp-agent；每条远程连接对应一个专属 "dsh --profile acp" 子进程
（进程边界 = 连接边界）。子进程命令由 --command 配置，生产默认 pnpm dsh --profile acp。

操作者节点伴生 acp-console：GUI 永远是"某个 peer 的控制台"，本地 WS 绑
127.0.0.1 + 随机 token（防浏览器 drive-by）；console 对 P2P 网络只做哑泵。

    ┌────────────────────────────┐             ┌──────────────────────────────┐
    │ acp-agent（常驻桥）          │   QUIC/TCP  │ acp-console（伴生进程）        │
    │  /dsh-acp/1 handler         │ ◄─────────► │  本地 WS 127.0.0.1+token     │
    │  策略门禁 + cwd 监狱         │  直连/中继   │  纯字节泵 WS ⇄ P2P 流         │
    │  每连接一个子进程             │             │  mDNS + rendezvous 发现       │
    │   dsh --profile acp         │             │ GUI（浏览器）→ 本地 WS 接入    │
    └────────────────────────────┘             └──────────────────────────────┘

构建与启动（两台机器都要做）：

    cargo build --release -p acp-agent -p acp-console -p p2pctl
    ./target/release/acp-agent --data-dir /var/lib/acp-agent --quic-port 7001
    ./target/release/acp-console --bootstrap <rendezvous地址> --ws-port 8087 --status-port 8088

console 启动即向 stdout 打一行 {"kind":"ready","ws":...,"status":...,"token":...,"peer":...}，
GUI/脚本从这里读端口与 token。

## 2. 端口与配置项全表

### acp-agent（JSON 配置文件 + CLI 覆盖；CLI 逐项覆盖文件值）

| CLI | 配置键 | 默认 | 说明 |
|---|---|---|---|
| --config | - | 无 | JSON 配置文件路径，字段可全部省略 |
| --data-dir | data_dir | ./acp-data | 桥数据目录：策略表/日志/身份都挂其下 |
| --quic-port | quic_port | 0（随机） | QUIC 监听端口 |
| --tcp-port | tcp_port | 0（随机） | TCP 监听端口 |
| --agent-name | agent_name | home-agent | ready 帧通告的 agent 名 |
| --command | command | pnpm dsh --profile acp | 每连接子进程命令行（空格切分） |
| --policy-path | policy_path | <data-dir>/acp-policy.json | 策略表路径 |
| --log-dir | log_dir | <data-dir>/acp-logs | 子进程 stderr 滚动日志目录 |
| --max-connections | max_connections | 8 | 连接总数上限 |
| --grace-secs | grace_secs | 10 | 断流后子进程宽限秒数（退出阶梯） |
| --sandbox-root | sandbox_root | <data-dir>/sandbox | scope=sandbox 监狱根 |
| --workspace-dir | workspace_dir | 无 | scope=workspace 锁定目录；未配置则该 scope 拒绝 |
| --reattach-window-secs | reattach_window_secs | 90 | 续连窗口（§12-Q1） |
| --permission-timeout-secs | permission_timeout_secs | 60 | ask 应答上限，超时 reject-once |
| --mcp-definitions-path | mcp_definitions_path | 无 | MCP 定义文件（名称 -> 服务定义 JSON） |

### acp-console

| CLI | 默认 | 说明 |
|---|---|---|
| --data-dir | ./acp-console-data | reattach 票据与 P2P 身份目录 |
| --bootstrap | 无（可多次） | rendezvous 引导地址 |
| --peer PEER@ADDR | 无（可多次） | 手动登记候选（直拨入口） |
| --no-mdns | 关 | 关闭局域网 mDNS 发现 |
| --agent-token | 无 | 透传给 agent 桥的握手 token |
| --ws-port | 0（随机） | 本地 WS 端口（只绑 127.0.0.1） |
| --status-port | 0（随机） | status HTTP 端口（只绑 127.0.0.1） |
| --window-secs | 90 | 断流续连窗口 |

### 端口暴露建议

- 对外暴露：agent 的 QUIC/TCP 端口（走中继则可不暴露任何端口）。
- 只留本机：console 的 WS/status 端口（127.0.0.1 + token 双条件，不要反代到公网）。

## 3. 策略管理（p2pctl acp，默认拒绝）

策略表 = <data-dir>/acp-policy.json，语义默认拒绝：表无条目的 peer 一律
peer-not-allowed 拒绝并写审计。授权即显式信任，撤销即回到默认拒绝。

    # 授予（upsert：重复执行即更新并刷新 granted_at）
    p2pctl acp allow <对方PeerId>       --scope sandbox                   # sandbox=每 peer 监狱（默认）/ workspace=锁定授权目录
      --ask-route remote_gui            # ask 权限路由：远程 GUI（默认）/ owner_local 本机
      --allow-mcp fs --allow-mcp git    # mcpServers 白名单（按名引用 host 预定义服务）
      --fingerprint <TOFU指纹>       --note "liu 的笔记本"       --data-dir /var/lib/acp-agent

    # 撤销（删除条目；本就无条目时明确报错）
    p2pctl acp deny <PeerId> --data-dir /var/lib/acp-agent

    # 列出全部授权
    p2pctl acp list --data-dir /var/lib/acp-agent

注意：p2pctl 默认 --data-dir ./p2p-data，与 acp-agent 默认 ./acp-data 不同——
两边必须指向同一策略文件，上面示例统一为 /var/lib/acp-agent。

TOFU（首次使用即信任）流程：

1. 陌生 peer 首连被拒，从审计日志取其 PeerId（conn-denied 事件）。
2. 通过二维码/线下渠道核对对方 PeerId 指纹（base58(sha256(公钥))，传输层握手已互认）。
3. acp allow 并 --fingerprint 显式登记；指纹入表备查，强制确认面归 GUI 波。

MCP 白名单语义（设计 §6，全案最重要一行）：白名单为空 = 远程 session/new.mcpServers
整字段剥离；白名单 peer 只能 {"name":"<名>"} 按名引用，且名字须同时在白名单与
node 配置（--mcp-definitions-path）里预定义——命令字节永远在 host 手里，违例整
请求拒绝（JSON-RPC -32602 mcp-servers-rejected）。

scope=workspace 必须同时在 acp-agent 配 --workspace-dir，否则该 peer 连接被
cwd-denied 拒绝。

## 4. 日志位置

| 内容 | 位置 | 说明 |
|---|---|---|
| 审计日志 | <data-dir>/acp-agent.log | 滚动文件；检索键 acp_audit |
| 子进程 stderr | <log-dir>/<peerId>-<conn>.log | 滚动 4 MiB x 3 份；每连接一份 |
| console 事件 | stdout JSON 行 | ready / state / discovery 三类 |
| console 状态 | GET /status、GET /discovery | Bearer token 鉴权，GUI 轮询用 |

审计事件清单（acp_audit target，只记 PeerId/错误码/时间，不含凭据与策略细节）：
conn-denied / gate-denied / conn-established / spawn-failed / client-gone /
subprocess-exit / cwd-denied / mcp-rewritten / permission-acted /
reattach-accepted / reattach-denied / window-expired / slot-superseded。
排障先看 conn-denied（谁被拒、什么码）与 subprocess-exit（退出状态或 killed-after-grace）。

## 5. 容量建议（设计 §7 资源门禁拍板）

| 项 | 值 | 配置 |
|---|---|---|
| 每 peer 并发连接 | 1（硬编码） | - |
| 每连接会话数 | ≤ 4 | 子进程 ACP 侧执行 |
| 连接总数 | 默认 8 | --max-connections |
| 每连接内存 | 约 150-300 MB（Node 子进程） | - |

8 GB 节点建议 ≤ 8 个并发控制台（默认值即按此取）；扩容前先算
max_connections x 300 MB + 系统底座是否留有余量。温池预热不共享状态，池化不破
隔离；单连接会话上限由 ACP 子进程保证，桥只做连接级门禁。

## 6. 故障矩阵摘录（设计 §7）

| 事件 | 桥行为 | GUI 看到 |
|---|---|---|
| 子进程启动失败 | 断流，错误入日志 | 连接失败提示 + 重试 |
| 子进程 mid-turn 崩溃 | 断流 | 错误气泡 + "尝试 resume"按钮 |
| 网络抖动（窗口内） | 续连 | 续连横幅 + 补放更新 |
| 网络断（窗口过期） | EOF→quiesce→持久化 | 引导走 session/resume |
| 非授权 peer 连入 | 握手拒绝 + 审计日志 | owner 侧日志可见 |
| 桥自身退出 | 全部子进程走退出阶梯 | 连接关闭；会话仍可 resume |
| 超限（单行>16MiB / 缓存>8MiB / 会话数>4） | 断流或拒绝 + 日志 | 明确错误，无静默 |

## 7. 停机与升级

- SIGTERM / Ctrl-C：先停监听，再向全部活槽位广播 Shutdown，各子进程按退出阶梯
  （stdin EOF → 宽限 → SIGKILL）收尾；宽限秒数 --grace-secs。
- 窗口内断流不杀子进程：session/update 进环形缓存（每会话 8 MiB），窗口内携票据
  重连即补放；窗口过期才降级为 quiesce + resume（§5 断线续连）。
- 升级 = 重启 acp-agent；已有会话凭子进程持久化可跨重启 session/resume，
  in-flight 那一轮不保证。
- 验收自测：apps/acp-agent 的 acp_wave_e2e 覆盖 §7 矩阵全部行；真 dsh 链路用例
  以 cargo test --test acp_wave_e2e -- --ignored --test-threads 1 单独跑，
  dsh 不可用时会打 SKIP 信号（不假绿）。
