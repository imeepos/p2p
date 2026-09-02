# 实验环境与多机验证方案

状态：E0 已完成（2026-09-02）。本文是 S 装配落地后的实验执行依据。
凭据红线：机器凭据只在 `.env`（已 gitignore），本文与任何脚本不得内联密钥。

## 1. 机器清单（连通性已验证 2026-09-02）

| 配置变量 | 内网地址/公网 IP | 系统/架构 | 工具链 | 实验角色 |
|---|---|---|---|---|
| MAC_SSH_15 | 192.168.0.15 | macOS arm64 (Darwin 25.6) | cargo 1.98 | 局域网节点 A（可编译） |
| MAC_LOCAL_SSH_114 | 192.168.0.114 | macOS arm64 (Darwin 25.5) | 无 cargo | 局域网节点 B（只跑产物） |
| LINUX_SSH_102 | 192.168.0.102 | Debian 13 x86_64 | cargo 1.97 | 局域网节点 C（可编译，异构 OS 验证） |
| LINUX_SSH_138 | 43.240.223.138 | Linux 5.15 x86_64（公网） | 无 cargo | 公网引导节点（rendezvous + relay + 打洞协调） |
| ECS_SSH_* | 121.196.193.177 | Ubuntu 26.04 x86_64（公网，阿里云） | cargo 1.98（部署脚本自装） | 第二公网引导节点（rendezvous + relay + 观测，凭据在 .env SSH_*） |

所有机器目录骨架 `~/p2p-lab/{bin,data,logs}` 已创建。SSH 密钥认证全部可用（BatchMode 验证）。

## 2. 拓扑

```
192.168.0.0/24（局域网，mDNS 生效域）              公网
┌────────────┐   mDNS   ┌────────────┐
│ 15  (macA) │◄────────►│ 114 (macB) │
│      ▲     │          └────────────┘
│      │mDNS │                    互联网
│      ▼     │   直连→打洞→中继   ┌─────────────┐
│ 102 (linC) │◄─────────────────►│ 138 (公网)  │
└────────────┘                   └─────────────┘
                                 rendezvous 注册/查询
                                 relay 中继、打洞协调
```

- 局域网三节点同网段，mDNS 自动发现直接生效（design §7.1）
- 138 常驻 bootstrap：局域网节点全部向它注册，跨网互查（design §7.2）
- 降级链实验路径：LAN 内直连；LAN↔138 方向上 138 有公网 IP 必直连；
  两条 LAN 节点经 138 中转时验证打洞（同时开洞）与中继兜底（design §7.3）

## 3. 阶段计划

| 阶段 | 内容 | 前置依赖 | 状态 |
|---|---|---|---|
| E0 环境就绪 | 连通性、目录骨架、本方案落档 | 无 | 完成（本轮） |
| E1 局域网实验 | 15/114/102 三节点 mDNS 互发现、echo roundtrip、断线事件；异构 OS（2×macOS + 1×Linux）兼容结论 | S(facade) 合并 + T 会话交付 p2p-cli 最小版 | 待启动 |
| E2 公网部署 | 138 部署 bootstrap 常驻（systemd），LAN 节点注册/查询打通 | D/R 已合并（已满足）；bootstrap 可执行文件（T 会话） | 待启动 |
| E3 跨网实验 | LAN 节点经 138 全链路：发现→直连→打洞→中继；打洞成功率采样 ≥20 次 | M3 贯通轮合入 + E2 | 待启动 |
| E4 长稳观察 | metrics 采集、断网重连、long-run 稳定性 | E3 | 待启动 |

## 4. 部署形态

- 138：`p2p-cli bootstrap` 以 systemd 常驻（`Restart=always`，日志进 journald + `~/p2p-lab/logs`）；
  138 无 cargo，产物两条路：a) 138 装 rustup 现场编译（默认，一条命令）；b) 102 上 musl 静态编译后 scp（备用）
- 局域网节点：实验期手工起进程 `p2p-cli node --data ~/p2p-lab/data/<name> --bootstrap <138地址>`，
  不配 systemd（避免实验期开机自启干扰）
- 产物分发：15 本机编译出 macOS arm64 产物分发给 114（同架构）；102 编译 Linux x86_64 自用
- 节点身份密钥落盘各机 `~/p2p-lab/data`（权限 0600，facade 已约定）

## 5. 实验验收清单

- E1：三节点两两 mDNS 互见 ≤30s；任一节点对另一节点 request/echo 成功；
  kill 对端进程后 ≤5s 收到 disconnected 事件；macOS 与 Linux 节点互通无平台分支
- E2：138 bootstrap systemd 开机自启、kill 后 5s 拉起；LAN 节点注册/查询公网 RTT < 200ms；
  安全组仅开放 QUIC/UDP 与 TCP 监听端口 + SSH
- E3：两条 LAN 节点经 138 互发现并建连；打洞成功/失败均有采样数据；
  打洞失败自动落到中继且上层 echo 依然成功（业务无感，design §7.3 降级链）
- E4：连续 24h 至少一条跨网链路存活；断网恢复自动重连（退避工具生效）

## 6. 安全红线

- 凭据与密钥只在 `.env` 与各机 `~/p2p-lab/data`，不入库、不进文档、不进脚本
- 138 只开必要端口；bootstrap 无状态（仅带 TTL 注册表），不落业务数据（design §1/§7.2）
- 实验期结束：kill 全部实验进程；rustup 等工具链保留供复用
- 每台机器的实验日志留 `~/p2p-lab/logs`，问题复盘时拉回主树 `docs/notes/`（脱敏）

## 7. 会话排期映射

| 会话 | 交付物 | 解锁 |
|---|---|---|
| S 编排装配（已完成） | swarm + Node facade | E1 的前置、T 的 node 子命令 |
| T 命令行（在途） | crates/p2p-cli：bootstrap/node/ping/discover | E1/E2/E3 的执行工具 |
| M3 贯通轮（S 后派单） | 降级链接入 swarm 拨号器 | E3 |

执行口径：S 落地 → 启动 T → T bootstrap 子命令先行 → E2 部署可与 T node 子命令并行 →
M3 贯通 → E1/E3 一起跑（先 LAN 后跨网）→ E4。

## 8. E1/E3 执行手册（runbook）

角色：15=macA（编译机）、114=macB、102=linC（编译机）、138=bootstrap（公网）。
机器凭据见 `.env` 对应变量；所有节点数据目录 `~/p2p-lab/data/<name>`，日志 `~/p2p-lab/logs/<name>.log`。

### 8.1 产物分发

```bash
# 15（macOS arm64，编译后分发 114）
ssh $MAC_SSH_15 'cd ~/src/p2p && export PATH=$HOME/.cargo/bin:$PATH && cargo build --release -p p2p-cli'
ssh $MAC_SSH_15 'scp ~/src/p2p/target/release/p2p-cli sker@192.168.0.114:~/p2p-lab/bin/'
# 102（Linux x86_64 自编译自用）
ssh $LINUX_SSH_102 'cd ~/src/p2p && export PATH=$HOME/.cargo/bin:$PATH && cargo build --release -p p2p-cli && install -m755 target/release/p2p-cli ~/p2p-lab/bin/'
```

源码上机：rsync -a --exclude .git --exclude target --exclude .worktrees --exclude .env ./ <host>:~/src/p2p/

### 8.2 E1 局域网实验步骤

1. 三台各起节点：`~/p2p-lab/bin/p2p-cli node --data ~/p2p-lab/data/<n> --name <n> 2>&1 | tee ~/p2p-lab/logs/<n>.log`
2. 判定 1（互发现）：≤30s 内三份日志均出现另两个节点的 PeerDiscovered
3. 判定 2（互通）：A 上 `p2p-cli ping <C 的 PeerId>` 成功且记录 RTT；再测 A↔B
4. 判定 3（断线感知）：kill 102 的进程，15/114 日志 ≤5s 出现 PeerDisconnected
5. 采集：三份日志拉回主树 `docs/notes/e1-results.md`（脱敏：只留 PeerId 前缀与 RTT）

### 8.3 E3 跨网实验步骤（依赖 E2 + M3 贯通）

1. 138 部署 bootstrap：`scripts/deploy-bootstrap-138.sh`，确认 systemd active 且监听 3400/udp+3401/tcp
2. LAN 三节点带 `--bootstrap 43.240.223.138:3400` 重启
3. 判定 1（跨网发现）：任一 LAN 节点 `p2p-cli discover` 能列出经 bootstrap 注册的其余节点
4. 判定 2（跨网互通+降级链）：LAN 节点两两 ping ≥20 次，逐次记录建立方式（直连/打洞/中继）与 RTT，
   打洞成功率 = 打洞成功数 / (打洞成功+失败回落中继数)
5. 判定 3（中继兜底）：任选一次交互中 `sudo systemctl stop p2p-bootstrap` 模拟引导节点失联，
   已建连接应继续可用；重新 start 后新发现恢复
6. 记录模板：`时间 | 链路(A→C) | 建立方式 | RTT(ms) | 备注`，写入 `docs/notes/e3-results.md`

### 8.4 判定与升级路径

- 任一判定不过：回传日志定位，能就地修的派回属主会话，跨 crate 的进协调裁决
- E1 全过 → M2 关闭；E3 全过 → M3 关闭，进 E4 长稳

### 8.5 双公网拓扑（138 + ECS，2026-09-02 部署）

第二公网引导节点 ECS（阿里云杭州，凭据只在 .env 的 SSH_HOST/SSH_USER/SSH_PASSWORD）与 138 并存，
端口组对齐：QUIC 3400/udp + TCP 3401/tcp + 观测反射 3402/udp。防火墙由阿里云安全组承担
（已放行 22/3400udp/3401tcp/3402udp），ufw 不启用。

部署入口 `scripts/deploy-bootstrap-ecs.sh`（幂等重部署）：root+密码经 SSH_ASKPASS 环境变量通道传递，
不进命令行参数、不落盘（需 OpenSSH >= 8.4）；worktree 内运行需 `DEPLOY_ENV_FILE=<主树>/.env`。
工具链从零自就位：apt 装 build-essential/curl/rsync，rustup 与 crates 均走 rsproxy 镜像
（sh.rustup.rs 从大陆云机常被重置，安装脚本也必须用镜像）。

冒烟记录（2026-09-02，15↔ECS，日志留存 102:~/p2p-lab/logs/ecs-smoke-102.log）：

- 部署面：bootstrap systemd active（NRestarts=0），PeerId 前缀 HNj8E7X5，三口监听齐全，
  15→ECS 跨公网 TCP 3401 可达
- 观测反射：节点经 ECS 学习到公网映射地址（UDP 空闲 12s 映射稳定不漂移）
- discover 经 ECS（QUIC /u3400）：列出全部注册节点与地址；rendezvous 链路 ~5s 生命周期 +
  30s 退避重注册为全系统常态（138 基线同节奏，注册表由此持续维持）
- ping 经 ECS：未通过——直连跳拨对端 NAT v4 观测地址被 home 路由器 hairpin 拒绝（refused）；
  CLI node 未接线 relay_addrs（无打洞信令/中继兜底），观测取首个成功反射（恒 v4），无 v6 路径。
  此为产品缺口而非 ECS 部署缺陷（CLI↔CLI 经 138 的 ping 同样受制）；解锁依赖：
  a) 安全组补 3403/udp + 3404/tcp（bootstrap relay 口）；b) p2p-cli node 暴露 --relay 接线（E4-S 范围）；
  c) 观测多反射器/v6 支持（crates 派单）
- TCP 引导（/t3401）：跨公网 TCP 握手成功但会话即断（read stream ended），存量问题已转 E4

节点接入：`p2p-cli node --bootstrap 121.196.193.177/u3400 --observation 121.196.193.177:3402`。
--bootstrap 当前为单值：双 bootstrap 冗余是部署级（切换 = 改 --bootstrap 重启），
单节点同时注册两个引导面待 CLI 支持多值后解锁。

### 8.6 E4 长稳采样脚本准备（T-ECS）

采样准备使用以下脚本，实际执行等待 relay 修复合入后进行：

- `scripts/e4-ping-sample.sh`：对指定 PeerId 连续采样，默认 3 轮；每轮流式记录 UTC、轮次、DialHop 路径、相邻 hop 到达间隔、hop 详情、RTT 和失败原因。
  三轮全部成功且路径一致才输出 `SAMPLE\tPASS`；结果文件为 TSV，可直接贴入协调表。默认 `RUST_LOG=info`，并显式传递双 bootstrap 与双 relay。
- `scripts/e4-sample-run.sh`：本机 A 节点编排器，提供 102/ECS 远端启动计划、PID 文件和采样调用；真实远端启动需显式设置 `E4_REMOTE_START=1`。
  节点 A 的 PID 保存在 `E4_RUN_DIR/node-a.pid`，清理只读取该 PID 后精确发送 TERM/KILL，禁止按进程名清理。

准备阶段自检：

```sh
scripts/e4-ping-sample.sh --self-check
scripts/e4-sample-run.sh --self-check
P2P_CLI_BIN=/bin/echo scripts/e4-ping-sample.sh --dry-run \
  --peer-id <PeerId> --bootstrap 43.240.223.138/u3400 \
  --bootstrap 121.196.193.177/u3400 --relay 43.240.223.138/u3403 \
  --relay 121.196.193.177/u3403
```

公网节点必须带 `--observation <公网IP>:3402`，否则注册表可能只暴露 `127.0.0.1` 监听地址，跨网拨号不可用。执行阶段记录每个目标的三连 TSV 和原始日志；三轮不一致或任一轮失败均判 FAIL，并把具体失败原因留在表中。
