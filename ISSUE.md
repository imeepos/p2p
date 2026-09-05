# ISSUE

## updater 密钥实为「空密码加密」而文档写无密码（2026-09-05 发布预检发现）

- **信息不准**：docs/ops/updater-release.md 写本机密钥「无密码」，实际密钥头解码为 `rsign encrypted secret key`（空密码加密）。本地 `tauri build` 不设 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 时签名步尝试 TTY 提示，报 `Device not configured (os error 6)`。
- **正确做法**：本地构建必须 `export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")" && export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""`（两者都要）。文档待更正。

## gui-client tag→release 路径从未成功过：GitHub secrets 缺失（2026-09-05 发布预检确认）

- **信息缺失**：updater-release.md 的 GitHub secrets 复选框未勾（需仓库管理员）。实证：client-v0.1.4 tag 的 gui-client run 33946473330，gate job 全绿，四平台构建 job 全部 failure 于「Tauri 打包」步（与本地缺密钥报错同因），故 v0.1.4 有 tag 无 release；0.1.0–0.1.3 的 release 均无 .sig/.tar.gz/latest.json，应用内更新端点 releases/latest/download/latest.json 一直 404。
- **影响**：secrets 登记前，任何 client-v* tag 都出不了 release。0.1.5 tag 推送被此阻塞。
- **期望**：管理员登记 `TAURI_SIGNING_PRIVATE_KEY`（= 私钥文件全文）后重打 tag；`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 留空不建（密钥为空密码）。

## ci.yml（ubuntu 全量门禁）在 main 存量红且本地 macOS 绿（2026-09-05 发布预检发现）

- **症状**：main 最近 5+ 提交（313061a 起）ci.yml 全部 failure，「全量门禁」步 28 秒级早退（exit code 2，来不及编译任何东西）；同一提交本机 macOS `make check` RC=0。
- **推断**：某个秒级子门禁在 Linux 上的平台假设（具体哪个待拿 CI 日志定位；已排除 /opt/homebrew 硬编码——check 脚本无此路径）。
- **期望**：有仓库日志权限者拉 run 33960564342 日志定位；修复前 CI 红不作为 macOS 本地发版的否决项，但属门禁体系债务。

## src-tauri 独立 workspace 不被根 fmt/clippy 门禁覆盖（2026-09-05 发现）

- **症状**：根 Cargo.toml `exclude = ["apps/gui/src-tauri"]`，`scripts/check/fmt.sh`（cargo fmt --check）与 clippy 门禁只扫根 workspace——PR1 合并带入 chat.rs fmt 漂移与 group_contract clippy 警告，make check 仍全绿。
- **期望**：fmt/clippy 门禁补跑 src-tauri workspace（或 CI 侧单列），盲区待收。

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

## DSH session_link_list 在 run_code 内绑定必败（2026-09-05 persona 轮发现）

- **症状**：run_code 内调用 `session_link_list`（无论传 `{}` 还是无参）一律报 `binding arguments must be lossless JSON`；session_link_talk/collect/create/send 同会话均正常。协调者被迫改用 git 探针（worktree/ls-remote）判断子会话进度。
- **期望**：修复该工具在 run_code SDK 层的参数序列化；或文档明示只能直调。

## DSH workspace_session_manage 单会话归档返回全工作区归档清单（2026-09-05 persona 轮发现，不可逆事故）

- **症状**：`archiveSession` 传单 sessionId（PR2 会话 b920c195），响应 `archivedSessionIds` 却返回 300+ 条——本工作区几乎全部历史会话被批量归档（含 E10/RS/IM 等历任协调会话）；第二次对 PR3 会话调用返回 387 条。按工具文档单会话形态只应归档一个。
- **影响**：归档不可逆（宿主无取消归档），其他协调线的历史会话被隐藏；幸运的是运行中的 PR1/PR3 会话与调用方自身未被归档。
- **期望**：查宿主实现是否把单 sessionId 误当过滤条件反向批量归档；修复前其他协调者慎用该工具。

## DSH session_link_collect 对运行中目标拒收 claimToken（2026-09-05 persona 轮发现）

- **症状**：talk 超时拿到 claimToken 后立刻 collect，报「会话历史中找不到该凭证对应的己方消息」；目标会话结束当前轮次后同一 token 即可正常收取。疑似运行中会话的消息历史未实时落盘。
- **期望**：文档补充「collect 仅在目标完成当前轮次后可用」，或实现运行中历史可读。

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

## 2026-09-05 R1 收尾观察：apps/cli SendLine 结构体 dead_code 告警
- cli-parity 重建 p2pctl 时报 `struct SendLine is never constructed`
  （apps/cli/src/group/send.rs:164，a430c40 basename 修复后输出改走 emit + JSON，
  旧文本摘要结构体遗留）。不拦门禁（cli-parity 对 warning 不敏感），
  但每次 make check 都刷一行噪音；建议该结构体随下次 CLI 输出重构一并删除或复用。

## 2026-09-05 R1 收尾观察：本地 main 与 origin/main 长期不同步
- R1 收尾时 origin/main 仍停在 a5a1bea，本地 main 已被并行会话推进至
  ccb7cd7 乃至其后继（acp-polish-page 系列合入）。若并行会话结束时未 push main，
  下一个会话 fetch 后会误判「落后远端」；建议各会话合并进本地 main 后尽快 push main，
  或在账本登记「本地领先远端 N 提交」的现状，避免下一会话基线误判。

## R1 修复后残余：不可达成员的积压条目被过早死信（2026-09-05 负责人复验发现）

- **症状**：三节点 p2pctl 复演——B 的 serve 进程僵死（监听在但不接受新连接）期间，owner 两条群消息对 B 投递失败；B serve 恢复后的下一次发送命令中，B 的两条积压条目被从 goutbox 移除但 B 始终未收到（历史永久 pending、队列已空、消息搁浅）。同序列对 C（正常重启）则完全正确：积压在紧邻命令内送达且 acks 回写。
- **机理推断**：R1 引入的内联补投与既有 spawn_outbox_task 后台 flush 在同一进程内对同一批 failed 条目各尝试一次，触发「每进程一次重投机会，二次死信出队」纪律——两次尝试都撞上不可达窗口即提前死信；itest 进程内时序连接正常故回归测试未覆盖此路径。
- **附带发现（底座域）**：长驻 serve 的 QUIC 监听会出现「进程在、端口在、新连接挂」的僵死态（两次演练各复现一次，B/C 各一次），重启 serve 恢复；疑似底座 accepted 路径问题，非群聊域，建议底座轮排查。
- **期望修法**：同一进程内联 flush 与后台 flush 共享同一尝试台账（或一次性命令模式停用后台 flush 仅留内联）；死信判定改为跨进程持久计数（条目 status 已落盘，可承载），不可达期间的积压禁止在单进程内死信。

## 33df7e4 修复登记（2026-09-05 R1.1，fix/group-goutbox-deadletter）
- 已修：goutbox 死信判定改跨进程持久尝试计数（GoutboxEntry.attempts 落盘，serde
  default 零迁移），attempt 单点记账——硬失败（连接成功但流/协议失败）计一次，
  连接失败/unknown_group 不计数；内联补投与后台 flush 经该落盘字段共享同一台账
  （方案一），删除组侧每进程 flush_tried 私账；内联改「先补积压(skip 本条)再投新条」。
  阈值 3：真毒条目 3 次硬失败即死信出队留告警，不可达窗口不消耗预算。
  回归 tests/group_deadletter.rs（同身份裸 Node 替身模拟 serve 僵死），红绿已验；
  commits e015c6d + 78776f2。
- 底座域附带发现（QUIC 长驻监听僵死）仍开放，待底座轮排查。
