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

## tests/smoke.rs 固定端口并行假红（T44 口径未覆盖，2026-09-05 G4 验收实测）
- **现象**：two_nodes_discover_ping_and_observe_dialhop 0.04s 即 FAILED：节点 b 启动 Address already in use (os error 48)——同机多会话并行跑各自节点测试时固定端口相撞；单跑即复绿。
- **现役吸收**：gui-tauri 门禁撞红先单测复跑鉴别环境散；尽量避免多会话同时跑 make check。
- **期望**：tests/smoke.rs 改端口 0 动态分配（T44 已修 chat 系测试，本文件漏网），src-tauri 测试域小改。
