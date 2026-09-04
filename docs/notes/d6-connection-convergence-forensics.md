# D6 取证：同 PeerId 多进程 churn 后「特定身份对」投递持续失败

> 2026-09-05 凌晨真机回归 R7 发现并完成机制定位。结论先行：**这不是连接池 bug，
> 是 dial.rs 连接收敛规则（E6/S2 冻结语义）与「同 PeerId 多进程」形态的结构性冲突**。
> 修复需要架构决策，本文件只做机制取证与选项分析，不改 swarm 代码。

## 1. 机制（dial.rs:207-254 自证）

收敛规则：双向拨号竞态产生两条连接时，**恒保留较小 PeerId 一端拨出的 Outbound 连接**
（keep_outbound = local_peer > remote_peer；prefer_new = (direction==Outbound)==keep_outbound），
落选连接显式 close。设计目的正当：防两端各留各的连接导致流与 serve 循环分家
（2026-09 GUI 闪断实测根因，dial.rs:213 注释自证）。

多进程同身份推演（B=4L2r… < A=8bA6…，B 为小 id 端）：

1. A 侧 one-shot 进程向 B 拨号：方向=Outbound、A 是大 id 端 → prefer_new=false；
   若 B 池已有 peer A 条目（无论来路），该新连接**落选被 close**。
2. one-shot 侧 QUIC 握手已成功、流被远端关闭 → 开流失败 → StreamFailed →
   chat 层 P2 重拨（仍 Outbound，仍落选）→ Failed。永久失败。
3. B 池中「错误方向」的在册条目（如池空期第一条 Inbound Accepted 所留）永驻：
   后续每条 Inbound prefer_new=false 全部落选；只有 B 自己拨出（Outbound）才能顶替。
4. 小 id 端（B）完全不受影响：B 拨出恒胜 → send/drain/flush 全链正常。

## 2. 真机证据链（R7，全部留档 /tmp/p2p-drill-evidence.log）

- B→A 背靠背 ×3：全 delivered，首条 flushedOutbox=3（小 id 端一切正常）
- A→B 单发与连发：全 failed（status=Failed，非 Pending——连接建立后的流级失败）
- 全新身份 C（同 Mac）→B：delivered——排除网络与 B 侧全局故障
- A→全新 serve D（102）：delivered——排除 A 数据目录毒性
- 重启 B serve：不愈——失败非 B 易失状态所致（方向规则重启后依旧成立）
- B 侧 messages：<peerA>.jsonl 7 rows / 7 unique / 0 dup——D2 修复未被破坏

## 3. 影响面

- 触发条件：同 data-dir（同身份）下「常驻进程 + 一次性进程」并存，且本端为大 id 端。
- CLI+GUI 共目录混用（p2pctl-ai-guide §5 明示支持）必然周期性踩中；纯 GUI 双实例
  （每身份单进程）不触发。
- 症状伪装成「节点坏了/消息丢了」，实际消息保留在 outbox，由小 id 端下次活动补投
  （flushedOutbox 可观测）。

## 4. 修复选项（需协调裁决，按侵入度排序）

| 选项 | 内容 | 权衡 |
|---|---|---|
| A CLI 进程内让位（最小） | CLI one-shot 启动时探测同目录常驻 serve，存在则经守护通道代发而非自建 swarm | 需新增 IPC 通道（daemon.sock 已有基建）；CLI 与 GUI 数据面统一，长期正确 |
| B 落选也 emit PeerConnected（一行语义加法） | 落选 close 前通知生命周期，让常驻侧 flush 得以借胜者连接补投 | 动 E6 冻结语义边缘；需论证不重开分家缺陷 |
| C 收敛规则加「连接世代」维度 | 同方向新连接替换旧连接（不区分方向） | 直接违背 E6 防分家设计，需重证 2026-09 闪断不复发，风险最高 |

建议：A 为目标态（根治），B 为过渡（一行 + 单测锁死不复活分家）。
