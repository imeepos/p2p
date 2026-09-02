# P2P 底座安全审查报告（第 2 期，E5 增量审计）

- 审查会话：E5（可观测性/长稳/安全加固轮）
- 基线：main @ 9c5c87d，对比 E1 安全审查（docs/notes/security-review-1.md，基线 45616a6）
- 范围：E4 期间新增代码（提交 4b34bc3/d10a427/9ea1a20、relay lifecycle 与 quic 修复）
  与实验/部署脚本、运维文档；生产代码按增量面复核，协议格式未变更
- 方法：只读逐文件审计 + 分派专项子代理（凭证与日志面/网络输入校验/资源耗尽面），
  高危项当轮修复或明确登记；行号以基线 commit 为准，修复后以新 commit 为准

## 结论速览

| 级别 | 数量 | 编号 | 处置 |
|---|---|---|---|
| 高 | 4 | H1-H4 | H1/H2/H3 当轮修复；H4 当轮修复 |
| 中 | 8 | M1-M8 | M1/M4/M5/M6 当轮修复；M2/M3/M7/M8 登记缓办 |
| 低 | 6 | L1-L6 | 全部登记缓办（有观测信号，长稳期监控） |

E4 增量未发现硬编码密钥/密码值、argv 数组执行路径无直接 shell 注入、
relay 帧长上限与 varint 溢出防护、电路/链路/全站配额均在位。

## 高危

### H1 采样脚本 askpass 密码临时文件与注入面（已修复）

- 位置：scripts/e4-sample-run.sh askpass_setup（基线 134-141）
- 问题：密码写入 `$ASKPASS_DIR/pw` 临时文件，且路径直接拼入生成的 shell 脚本
  `cat '$ASKPASS_DIR/pw'`；TMPDIR 可控时含引号/命令片段可在 askpass 触发时执行注入；
  与 experiment-env.md「密码不落盘」承诺矛盾。
- 修复（4ab79fe 前置的脚本修复，见 fix(scripts) 提交）：askpass 脚本改为只引用
  `$SSH_PASSWORD` 环境变量（`printf '%s' "$SSH_PASSWORD"`），密码全程不落盘、
  无路径拼接；与 deploy-bootstrap-ecs.sh 同构。

### H2 ssh 目标未校验，可选项注入（已修复）

- 位置：scripts/e4-sample-run.sh SSH_B/SSH_T（基线 20-21、142-145）
- 问题：目标串未经校验直接作为 ssh 首参数，形如 `-oProxyCommand=...` 可被解析为
  选项，改变网络边界；`StrictHostKeyChecking=accept-new` 自动信任首见 host key。
- 修复：新增 valid_ssh（`user@host` 形态白名单正则，拒绝 `-` 开头与空白），
  check_config 强制校验；accept-new 保留（实验机重装场景），已知主机指纹固定
  列为后续加固（L6）。

### H3 pidfile 无进程身份绑定，PID 复用可误杀（已修复）

- 位置：scripts/e4-sample-run.sh stop_local（基线 115-131）
- 问题：仅数字校验后直接 TERM/KILL；进程退出后 PID 被无关进程复用时可能误杀。
- 修复：kill 前以 `ps -p <pid> -o comm=` 核对进程名必须为 p2p-cli（basename 比对，
  覆盖 macOS 全路径与 Linux 短名），不符即按陈旧 pidfile 处理不杀。

### H4 rendezvous query 只读路径可扩表，任意连接可撑大服务端内存（已修复）

- 位置：crates/p2p-discovery/src/rendezvous/server.rs query()（基线 72-85）
- 问题：query 对任意 namespace `entry().or_default()` 创建缓存条目，无 namespace
  校验、无限速；任意已建立连接循环发送唯一 namespace 即可持续创建 HashMap 条目，
  内存耗尽。register 的非空/64 字节校验未覆盖 query 路径。
- 修复（fix(discovery) 提交）：query 改为纯读——非法（空/超长）与未知 namespace
  一律返回空结果，绝不创建条目；回归
  `query_unknown_namespace_does_not_grow_registry` 断言注册表零增长。
  登记限速（M8）不在本条范围内，另行登记。

## 中危

### M1 采样脚本输入无上限，直调可放大资源消耗（已修复）

- 位置：scripts/e4-ping-sample.sh（基线 10、146-157）
- 问题：RUNS/WAIT 无范围上限，ROWS 全量累积内存，RAW_LOG 无限追加。
- 修复：check_args 强制 RUNS 1..1000、WAIT 1..300；umask 077 落盘 0600；
  OUT/RAW_LOG 拒绝符号链接。ROWS 累积保留（上限内规模可控），流式改写登记 L1。

### M2 relay reject_link 无超时/流数上限（缓办）

- 位置：crates/p2p-relay/src/service.rs reject_link
- 问题：超配额链路持续 accept_stream，无 idle 超时或数量上限，可被大量连接钉住
  任务/FD。修复涉及断链行为变更，需要长稳/拓扑数据支撑，登记 E6 候选；
  期间由 relay 链路配额（每 peer 8/全站总量）与进程 FD 上限兜底。

### M3 punch 信令转发无节流（缓办）

- 位置：crates/p2p-relay/src/control.rs forward_punch
- 问题：信令转发无节流；盲拨面（relay/rendezvous 链接 expected=None）下身份轮换
  可稀释 per-peer 配额放大信令。限速属行为变更且需要真实打洞成功率数据定参，
  登记缓办；观测面已有 punch/relay 跳计数可量化。

### M4 输出文件权限与符号链接（已修复）

- 位置：scripts/e4-ping-sample.sh OUT/RAW_LOG、scripts/e4-sample-run.sh RUN_DIR
- 修复：umask 077 + symlink 拒绝 + RUN_DIR 0700（见 M1 同一提交）。

### M5 部署/采样文档口径与实现不一致（已修复）

- 位置：docs/ops/experiment-env.md 8.5/8.5.1
- 修复：补记 ECS sshd 加固事实（密码登录已禁用）、密码「不落盘」承诺现与实现
  一致（H1 修复后无密码临时文件）、日志保留与脱敏要求（同机 0600、回传主树前
  只留 PeerId 前缀与 RTT）。

### M6 公网节点密码/root 登录（已修复，独立提交 chore(ops)）

- 138 复核本就 `PermitRootLogin prohibit-password` + `PasswordAuthentication no`；
- ECS 原为 root+密码（cloud-init drop-in 曾覆盖主配置），已装公钥并改
  `PasswordAuthentication no` + `PermitRootLogin prohibit-password`，另加
  `/etc/cloud/cloud.cfg.d/99-e5-disable-pwauth.cfg` 防重启回退；
- 验证：密码登录 Permission denied（仅 publickey），BatchMode 密钥登录可用；
  改动前配置备份在远端 `/tmp/*bak*`。

### M7 地址校验允许任意主机名（部分修复）

- 修复：端口范围 1..65535 强制（两脚本）。主机名/CIDR 白名单未做——采样脚本
  目标由实验者显式传入，受信环境风险可接受；登记至 L4 一并评估。

### M8 rendezvous 查询无限速、Register 大帧（缓办）

- LengthDelimitedCodec 默认 8MiB 帧上限偏大、地址列表无条数上限；限速与帧上限
  调整属协议行为变更，登记 E6 候选（与 M2/M3 同批定参）。

## 低危（登记，长稳期监控）

- L1 采样 ROWS 内存累积与 RAW_LOG 无轮转：上限内（RUNS<=1000）可控；超大规模
  采样前改流式。
- L2 wait_ready 就绪判定依赖日志 grep：日志已按启动截断，误判面小；后续可加
  metrics 端点探活。
- L3 itest 测试管道任务无上限/超时：仅测试 harness。
- L4 采样目标无网段白名单（SSRF 面）：实验脚本受信环境；如复用于不受信场景
  必须先加允许列表。
- L5 relay joiner 配额在桥接异常路径可能迟回吐（slots.rs）：有全站配额兜底，
  长稳期以 relay metrics 水位观察，增长异常再修。
- L6 ssh known_hosts 固定指纹未做：两公网机 host key 已落本机 known_hosts，
  固化指纹随下次运维窗口执行。

## 与第 1 期（security-review-1.md）的衔接

- 第 1 期 H1（rendezvous 注册签名未覆盖 TTL）：E4 已由签名载荷扩展修复
  （sign_register 覆盖 ttl 与时间戳，tests.rs tampered_ttl_replay_rejected 回归在位）。
- 第 1 期 M1（rendezvous 无资源上限）：已部分落地（namespace 长度/单表 peer 数/
  TTL 上限/注册限速）；本轮 H4 堵住 query 扩表旁路，M8 余项缓办。
- 第 1 期 M2（电路 cid 可枚举）：已由 CSPRNG cid + allowed_joiner 校验修复。
- 第 1 期 M4（dial expected 为 Option）：swarm 拨号路径已类型层强制 Some（dial.rs
  安全不变式注释在位），盲拨仅存于 relay/rendezvous 链接并留 WARN 日志。
- 第 1 期 M3（握手超时）与 M5（配额 Sybil 稀释）：部分落地（quic idle、全站配额），
  余项与 M2/M3/M8 同批缓办。
