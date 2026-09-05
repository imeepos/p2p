# ISSUE

## AGENTS.md 远端名与实际不符（2026-09-04 T36 检查轮发现）

- **信息不准**：AGENTS.md「收尾四步」一节写「远端名是 gitea 不是 origin」并示例 `git push gitea <分支>`；但本仓库实际只配置了 origin 远端（git@github.com:imeepos/p2p.git），不存在 gitea。账本 note（2026-09-04 12:30 CLI 对等波勘误）已明确「本仓库远端实为 origin（无 gitea），后续任务书统一用 origin」。
- **正确做法**：推送/删远端分支一律用 `git push origin ...`；AGENTS.md 待同步更正。

## 底座 facade 契约缺口：ProtocolHandler 拿不到入站流 PeerId（2026-09-05 ACP2 发现）

- **信息缺失**：crates/p2p-protocol 的 `ProtocolHandler::handle(&self, stream)` 只回调流，不传远端 PeerId；而 crates/p2p-swarm 的 serve.rs 在 dispatch 时明明持有 `peer`（喂给了 liveness/事件，唯独没进 handler）。设计（acp-over-p2p-design.md §4.1 ①）要求桥按「传输层互认 PeerId」查策略表，目前上层 app 无法直接拿到。
- **ACP2 的绕行**：apps/acp-agent/src/peers.rs 以 Node 事件（PeerConnected/PeerDisconnected）维护在线集，恰一 peer 在线才归属，空集短等、多 peer 歧义一律 fail-closed 拒绝并审计；单操作者场景正确，多操作者并发控制台会被误拒（有日志）。
- **期望修法**：底座把 peer 随流下传（trait 加参或 BoxedStream 携带元数据），acp-agent 删 peers.rs 直连真实身份；涉及 crates/**，非 ACP2 文件域，留待底座卡。


## 主树 git merge-base --is-ancestor 无锁挂起（2026-09-05 E10 收编轮发现）

- **症状**：主树执行 `git merge-base --is-ancestor <commit> main` 无任何锁（.git/*.lock 为空、无 gc/repack 进程）持续挂起，gtimeout 10s 杀出 rc=124；同仓 worktree 内与 rev-parse/log/ls-tree 等其余 git 命令均正常。复现 2 次，未破案。
- **绕行**：祖先判定改用 `git ls-tree main --name-only`（产物级判据）或 `git log main | grep <hash>`；已在 E10-T20/T21 验收链避免使用 merge-base。
- **期望**：观察是否复现；若复现扩大，用 `GIT_TRACE_PERFORMANCE=1` 与 commit-graph 重建（`git commit-graph write --reachable`）定位。
- **后续观察（2026-09-05 同日）**：同命令对同仓重执行 rc=0 正常返回，判定为瞬时资源竞争类（当时多会话并行 git 操作密集），暂不升级；判据绕行继续沿用。

---

## p2pctl-ai-guide.md 操作性缺口（2026-09-04 AI 试运行发现，详情 docs/notes/ai-pilot-findings.md）

- **信息缺失**：无「两节点聊天最小拓扑」章节——chat 收发用 chat 身份（chat serve 输出）而非守护身份、接收方须 chat serve 常驻、friends add --addr 格式，均需试错才能拼出（§1.3 只陈述现象）。
- **信息缺失**：identity.lock 未记载——chat serve 与 chat send 同 data-dir 互斥（§1.3 守护信号只列 daemon.*）；建议补锁清单+互斥矩阵；另缺「查本机 chat 身份」的离线只读命令。
- **信息不准**：§1.4/gui screenshot 前置缺「macOS 屏幕录制授权」（实测 CAPTURE_PERMISSION_DENIED 直接判死 screenshot 与 ui-regression.sh）；gui page 条目称 dashboard 未注册会 PAGE_NOT_REGISTERED，实测 dashboard 正常返回 descriptor。


## 底座 rendezvous 近似单次服务 + facade 注册观测退化（2026-09-05 T23 两机冒烟发现）

- **现象 1**：rendezvous 服务端对同一连接近似只服务一次——第二个借方进程查号挂 10s 握手超时，bootstrap 侧每 10s 一条 server link ended 日志。
- **现象 2**：facade 装配期观测单次随机失败导致注册退化为 loopback 地址。
- **现役吸收**：T23 冒烟在 harness 层以「S2/S3/S4 单进程合并 + 发现窗口 60s + rc=2 有界重试 + 预观测等待」规避；产品 E2E（T20）同机回环不受影响。
- **期望**：底座层修复单复核（rendezvous 连接复用/多查号；facade 注册重试与地址优选），涉及 crates/p2p-discovery 与 p2p-swarm，属底座卡非应用层。

---

## AGENTS.md 收尾四步引用的 gitea 远端在本机主克隆不存在（2026-09-05 ACP-P1/P2 收尾实测）

- **现象**：AGENTS.md 收尾四步写「git push gitea 分支（远端名是 gitea 不是 origin）」，但 /Users/imeepos/ext512/p2p 主克隆 git remote -v 只有 origin（github.com:imeepos/p2p.git），无 gitea 配置；并行会话按四步执行 push gitea 时无从落地（本地 ff-merge 与 worktree/分支清理均已完成，无代码损失）。
- **现役吸收**：负责人收尾改推 git push origin main 保代码安全（main 已与 origin/main 同步）。
- **期望**：明确 gitea 实例地址（疑似 102 服务器）并在主克隆补配 remote，或修订 AGENTS.md 收尾四步的远端名口径。

---

## tests/smoke.rs 固定端口并行假红（T44 口径未覆盖，2026-09-05 G4 验收实测）
- **现象**：two_nodes_discover_ping_and_observe_dialhop 0.04s 即 FAILED：节点 b 启动 Address already in use (os error 48)——同机多会话并行跑各自节点测试时固定端口相撞；单跑即复绿。
- **现役吸收**：gui-tauri 门禁撞红先单测复跑鉴别环境散；尽量避免多会话同时跑 make check。
- **期望**：tests/smoke.rs 改端口 0 动态分配（T44 已修 chat 系测试，本文件漏网），src-tauri 测试域小改。
---
## 2026-09-05 同 peerId 重连被对端半开连接残留挡下（底座）
- 现象：同一身份第一次进程拨对端成功；该进程退出后，同 peerId 的任何新进程
  再拨同一对端（对端进程一直存活）一律 ConnectFailed（send/邀请重投同现）。
- 影响：CLI 一次性命令模型下第二次投递必失败；邀请自愈收敛依赖重连，被挡。
- 复现：scripts/ops/cli-friend-invite-e2e.sh 注释（编排绕开：对端重启清残留）。
- 疑点：p2p-swarm pool/liveness 对死连接的半开判定缺失，B 端按 peerId 拒新连接。
- 待办：底座补半开检测或入站新连接替换死连接；修复后 e2e 可去掉固定端口与轮转。

## 2026-09-05 旧 friends add 语义脚本待改造
- scripts/ops/cli-chat-e2e.sh、cli-live-e2e.sh、cli-gui-data-e2e.sh、
  cli-chat-concurrency-e2e.sh、cli-friends-race-e2e.sh 仍按旧直加语义编排，
  邀请制下需改造（新增 cli-friend-invite-e2e.sh 已覆盖核心邀请流）。不在 make check，
  opt-in 执行前必须先迁移，避免假绿。

## IM 群聊一次性命令拓扑演练缺陷（2026-09-05 负责人 p2pctl 三节点实跑发现）

- **补投时机不稳定**：owner 一次性 `group send` 对离线成员产生 pending 条目后，紧邻的后续发送未触发补投（收端缺消息），隔数次命令后才送达——疑似一次性进程退出与 outbox flush 任务竞态（im-group-drill 实录：『离线补投』两次触发均未即时补投，数步后才到）。演练清单 §4 期望『重连即 flush』。
- **sender 侧 acks 记账脱节**：goutbox 补投/重发成功后，发送端群历史条目的 acks 不更新（『第一条』实际已全员送达，A 端永远 pending/acks 缺 C）；goutbox 条目 status=failed 但内容实际已送达，双账本互斥。
- **failed roster 条目不重试**：rename 的 roster 推送对某成员 connection lost 后条目滞留 goutbox status=failed，后续连接不补投；该成员停留旧 rev 直至下一次 roster bump 才收敛（高 rev 胜兜底了最终一致，中间窗口视图过期）。与演练清单『已知边界：roster/通知不丢失』不符。
- **CLI --file 默认显示名取全路径**：`group send --file /x/y/shot.png` 的 media.name 为整条路径，sanitize 后成 `tmpim-group-drillshot.png`；应取 basename（help 文案写『默认取文件名』）。
- **演练清单拓扑盲区**：D6 身份互斥使『三方 serve 常驻』与『B/C 自有一次性命令』不可同时成立；且 owner 纯一次性拓扑下成员→owner 的 G_LEAVE 无通路（成员簿无 owner 地址可拨、入站连接不触发 flush），演练清单 §5『重邀回归』在该拓扑下必撞『已在群中』假错误。清单需补混合拓扑操作序列（owner 操作与成员 serve 启停的交错步骤）。
- **验证为正常的部分**：建群/入群 roster、文本与附件 fan-out（acked n/n、字节 sha 一致）、rename 高 rev 收敛、kick/leave/disband 状态迁移与拒发、解散不删数据、未全员送达退出码 1。
