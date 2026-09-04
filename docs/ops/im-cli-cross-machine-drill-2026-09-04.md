# IM 聊天跨机 CLI 演练记录（Mac15 ⇄ Linux102）

> 日期 2026-09-04。二进制基线 main @ 004b441 时代构建（会话期间 main 推进至 ffc0f0d，
> 报告落盘时未回捞重测；其间合入的 067b7ff 发送失败热fix针对 GUI 旧版本无 /im/chat/1
> 的版本偏差问题，与本报告 D1 竞态非同一缺陷）。
> 形态：双端纯 p2pctl（apps/cli），非 GUI。对应演练清单 docs/ops/im-chat-drill.md 的
> CLI 可执行子集；GUI 专属项（asset 预览/播放/视觉）不在本记录范围。

## 1. 拓扑与方法

| 项 | 节点 A | 节点 B |
|---|---|---|
| 机器 | Mac mini 192.168.0.15（本机） | Debian 12 x86_64 192.168.0.102（ssh） |
| 二进制 | apps/cli/target/debug/p2pctl 0.1.0 | 同源 rsync 后 102 本地 cargo 构建（ELF） |
| 身份 | FYnw1Tq3zqGHGEQUBmCXeZqmtqPxAh3kEYZ9zqbvZKHN | EThqg9SsUEBAE6sNxtKwZTfr3igSf2Gr6sHw12Q5maek |
| 数据目录 | /tmp/p2p-drill-a（隔离） | /tmp/p2p-drill-b（隔离） |
| QUIC 端口 | 固定 35101 | 固定 35102 |

- 发现方式：无 mDNS/rendezvous，纯好友簿地址直连（addr=对端 LAN IP + QUIC 端口）。
  swarm 实际绑定 0.0.0.0（p2p-swarm/src/swarm/mod.rs:139），listen_addrs 报告的
  127.0.0.1 仅是展示替换（config.rs to_transport），跨机取「报告端口 + 对端 LAN IP」即可直连。
- 原始证据：/tmp/p2p-drill-evidence.log（本机，易失，关键行已摘录进本记录）。

## 2. 结果总表（演练清单 §6 模板）

| 项 | 结果 | 备注 |
|---|---|---|
| 2.1 文本互通 | 通过 | A→B、B→A 双向 delivered，B 侧 serve 实时 chat_message 事件 + history 落盘双证 |
| 2.2 emoji | 通过 | 首发命中 D1 报 failed（内容无罪，单独重发 delivered），复测 UTF-8 原样保真 |
| 2.3 图片 | 通过 | pixel.png 70B，sha256 逐位一致 |
| 2.4 音频 | 通过 | tone.wav 46B，sha256 一致 |
| 2.5 视频 | 通过 | clip.mp4 2KiB，sha256 一致 |
| 2.6 任意文件 | 通过 | note.txt 双向（A→B、B→A）sha256 一致 |
| 2.7 状态流转 | 通过（带 D1 例外） | 在线发送 pending→delivered 正常；例外见 D1/D3 |
| 2.8 离线投递 | 最终通过（严重降级） | 「不丢」底线成立；补发链路病态，见 D1/D2/D4 |
| 2.9 历史分页 | 通过 | limit=3 + before-id 游标严格更早，无重复无遗漏 |
| 2.10 回复引用 | 通过 | replyTo 跨机透传落盘，指向被引用消息 id |
| 负路径 5 例 | 通过 | 非法 PeerId/自加好友/坏 base58/附件不存在/text+file 互斥，全部可读中文错误 + exit 1 |

## 3. 缺陷与发现（按严重度）

### D1 发送路径竞态：前连接关闭后数秒内的下一次发送报 failed（高频）

- 现象：B→A 连续背靠背发送，3/3 轮复现（rapid-1/2/3-b 全 failed）；emoji 首发、
  B 重启后 poke 全部 failed。同间隔 A→B 从未复现（方向不对称未定论）。
- 双端状态分歧：部分 failed 消息对端实际已收到（rapid-1-b/2-b 在 A 侧 delivered，
  B 侧记 failed）——发送端报告与对端事实相反。
- 已定位的机制线索（供修复轮取证，非定论）：
  - mark_failed 路径 core.rs:147 与 lib.rs:214：非 ConnectFailed 的投递错误即翻 failed；
  - flush 重投（core.rs:169 flush_peer）持有 peer 串行锁，与 send 同锁（lib.rs:204），
    注释自证「同连接并发开流（yamux 上游缺陷）」的规避手段在多进程场景失效；
  - 一个 PeerId 多进程共享（one-shot 反复拉起 + 常驻 serve 并存）时，连接复用可路由到
    正在死亡进程所持有的连接 → 开流/ACK 失败。
- 复现配方：同 data-dir 连续两条 chat send（间隔 < 5s），第二条高概率 failed。

### D2 消息同 id 双行：状态更新为追加、history 不按 id 去重

- 证据：B 侧 messages/<peer>.jsonl 中 0466a1b2 出现两行（status=failed 与 delivered），
  chat history 输出两条同 id 记录。GUI/CLI 展示层若不去重即双重气泡 + 状态翻转闪现。
- 位置：store 的 update_message_status 落盘形态与 messages_for 读取未按 id 归并。

### D3 failed/pending 条目静默复活：报告与最终事实不一致，且无通知

- 报 failed 的消息后续经 outbox 重投送达对端（emoji 首发、rapid-1-b/2-b、rapid-3-b 均如此），
  发送端 CLI 已退出，用户视角永远是「失败」；接收端则多出消息。pending 同理（84b6d51e
  首次补发尝试失败后滞留）。设计上「条目保留待重发」合理，但缺终态回写与可观测信号。

### D4 outbox 失败条目堆积毒化后续发送（本次最重发现）

- 现象：B 的 outbox 堆至 8 条 failed 后，B→A 任何发送 100% failed（含隔离单独发送）；
  将 outbox/<peer>.jsonl 移走后立即恢复 delivered。同机干净身份（fresh data-dir）发送正常，
  排除网络/二进制因素，毒源即 outbox 状态文件。
- 机理指向：每次连接触发 flush_peer 重投全部条目（含 failed），堆积后与 send 的锁争用/
  顺序投递阻塞拖垮新消息（与 D1 同根）。
- 运维启示：遇「该节点外拨全 failed」先查 outbox/<peer>.jsonl 行数。

### D5 one-shot 生命周期与 flush 任务竞争：补发不可靠

- chat send 成功路径发完即退，PeerConnected 触发的 flush 任务常被进程退出杀死；
  84b6d51e 的补发最终达成依赖「失败发送的 30s wait_outbox_flush 重连窗口」这一副作用。
- CLI 下离线补发无可靠触发方式（serve 常驻端在收到对端拨入时 flush 才可靠）。

## 4. 校准项（文档/实现表述差）

1. 指南称 --kind 默认「按载荷推断」，实际 CLI 默认恒 File（apps/cli/src/chat/payload.rs:30，
   有单测钉死）；mime 才按扩展名推断。结果：pixel.png 以 kind=file + mime=image/png 落库。
   GUI 按 mime 白名单归 image/audio/video，CLI 需显式 --kind image|audio|video 才一致。
   建议改指南措辞或在 CLI 按 mime 推断 kind（后者动契约，需裁决）。
2. p2pctl 源自用 chat serve（非 node start）：两套身份根（chat 域 vs 守护 dataDir），
   本次 chat serve 输出 peerId 即聊天身份，符合指南 §1.3，演练无歧义。

## 5. 数据完整性结论

- 文本/emoji/四类媒体跨机双向字节级一致（sha256 全对）；64MiB 上限与 MIME 白名单
  拒绝路径未在本次真机覆盖（T29 itest 已覆盖，真机仅验通过路径）。
- 重启持久化：B 重启后 peerId/好友簿/端口（固定 QUIC）全部保持；A/B 历史完整回读。
- 全程零消息丢失：所有 pending/failed 条目最终送达或在隔离数据目录中可见。

## 6. 清理记录

- A/B serve 进程已停（SIGTERM，退出路径正常）；102 侧 /tmp/p2p-drill-b、/tmp/p2p-clean-c、
  /tmp/p2p-outbox-quarantine 已删；~/p2p-drill-src 构建树保留（102 无 cargo 缓存重建需 2m09s，
  留作后续演练）。本机 /tmp/p2p-drill-a、夹具与证据日志保留至本记录合入后可清理。
- 102 构建耗时参考：干净全量 2m09s（12 核，debug profile）。

## 7. 修复轮回归补记（2026-09-05，fix/im-cli-outbox-resilience @ f278e2d）

D1-D5 修复（dc03f31/734740a/fa3a003）后以同拓扑重放（节点 8bA6…qY3er ⇄ 9Xht…cvkRm，
端口 35201/35202）：

| 项 | 结果 | 证据 |
|---|---|---|
| R1 B→A 背靠背 ×3（D1 原必现失败面） | 通过 | 全部 delivered；首条 flushedOutbox=3 顺手补投 3 条历史 pending |
| R2 A→B 单发+连发 | **失败（D6 新登记）** | 见下 |
| D2 同 id 双行 | 通过 | B 侧 messages 7 rows / 7 unique / 0 dup |
| flushedOutbox 字段 | 通过 | send JSON 实测出现（契约加法生效） |
| A→全新 serve D | 通过 | delivered——证明 A 目录无毒、Mac→102 路径健康 |
| 全新身份 C→B | 通过 | 证明 B serve 对新 peer 正常 |

### D6（新登记，顶层待裁决）：同 PeerId 多进程 churn 后「特定身份对」投递持续失败

现象：`(A 身份, B serve)` 组合一旦进入坏态，A→B 全部 failed（connect 后传输层失败，
重拨一次仍败）；同机全新身份→B 即通；A→第三方 serve 即通；**重启 B serve 不愈**、
A 数据目录换新即愈。首日演练的镜像现象（B→A 全灭而 A→B 正常）同属此面。

指向：swarm 连接门禁/连接池按 PeerId 维度跟踪的对端状态，在「同身份多进程 + 高频
一次性连接 churn」下对后续连接做出拒绝/失活判定，且该状态跨 serve 重启仍可被快速
再触发（具体判定路径需 swarm 层取证：ConnectionGate 与连接池键控，E6/S2 面）。

影响面：CLI one-shot 与常驻 serve 同 data-dir 混用（指南鼓励的 CLI+GUI 共目录形态）
必然踩中；纯 GUI 双实例不触发（每身份单进程）。

处置建议：立 swarm 层修复单（连接门禁对同 peer 新连接应替换陈旧跟踪而非拒绝，
或按 conn 世代失效）；修复前 CLI 侧规避=坏对两端均换新 data-dir 身份。
