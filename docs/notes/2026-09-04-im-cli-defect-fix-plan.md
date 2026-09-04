# IM CLI 跨机演练缺陷修复计划（D1-D5）

> 依据：docs/ops/im-cli-cross-machine-drill-2026-09-04.md（真机取证）。
> 社区实践对标（检索确认方向）：
> - MQTT5 会话队列纪律：离线队列必须**有界**（消息过期/最大排队/丢弃策略），无界重投即毒化。
> - 事务性 outbox / 毒消息惯例（RabbitMQ/MassTransit 族）：**最大投递次数 + 死信**（脱离投递队列，
>   保留记录），重试带退避；接收端以消息 id 幂等去重（本仓 wire §8 已有）。
> - libp2p request-response：每次请求**有界超时**，连接按 peer 池化但应用层对失败流重拨新连接。
> - append-only 日志：读侧按 key last-wins 归并 + 择机压实；更新判定以磁盘真值为准（防跨进程丢失更新）。

## 修复项（按序执行，每项测试+独立提交）

| 项 | 缺陷 | 方案 | 验收 |
|---|---|---|---|
| P1 | D2 同 id 双行 | store 更新判定改磁盘真值：update_message_status/set_outbox_status 返回 bool(命中行数)；mark_delivered 以「磁盘未命中」才走追加分支；messages/outbox 装载时按 id last-wins 归并 | 单测：双行文件读回=1 条最新态；delivered 判定不再产生第二行 |
| P2 | D1 投递竞态 | core.deliver 失败（非 ConnectFailed）时 disconnect+重连一次再试一次（全程 peer guard 内）；deliver 全程 tokio 超时预算（ACK 等待有界） | itest：背靠背三轮 rapid-fire 全 delivered |
| P3 | D4 outbox 毒化 | flush_peer 纪律：Failed 条目每进程最多重投一次（内存集合记账），再失败即出队（死信：消息记录保留 Failed + warn）；单次 flush 批量上限（32），约束 guard 占用 | itest：outbox 含 stuck failed 条目时新发送不受阻；条目最终出队 |
| P4 | D5 one-shot 不排队 | Chat::drain_peer(peer, budget)：send 成功后 CLI 一次性命令内 best-effort 排空该 peer outbox（预算 10s） | itest：对离线 peer 队列 1 条，重新连线后一条 send 返回即双条送达 |
| P5 | D3 无观测 | ChatSendReport 增 additive 可选字段 flushedOutbox（本轮排空的既有条目数，serde default）；CLI --json/text 输出 | 契约加法登记 gui-contract §12；CLI 冒烟断言字段 |
| P6 | 指南措辞 | ai-guide --kind 默认值描述修正（mime 按扩展名推断；kind 默认 file，可显式覆盖） | make check ai-docs-sync 门禁绿 |
| P7 | 回归验收 | 102 真机重放：rapid-fire 三轮、离线补发流、outbox 毒化恢复 | 全 delivered + 演练记录补记 |

## 边界与纪律

- 不改 wire 格式（无帧变更）；不动 swarm/relay（E6/E8 已收口面），重拨在 chat 层完成。
- ChatSendReport 加字段走契约加法（serde(default)），gui-contract 独立小提交。
- 每项 fix+回归同提交；合并前 worktree 内全量 make check；P7 真机验收通过才算闭环。
