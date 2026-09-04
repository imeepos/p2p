# yamux 0.13.10 流语义实测发现与治理建议（ACP3 泵挂死定位副产物）

> 日期: 2026-09-05 | 来源: apps/acp-console WS⇄P2P 字节泵 4/5 随机挂死定位
> 范围: crates/p2p-mux（yamux 0.13.10）行为画像；本文件只记录事实与建议，不改底座。

## F1 半关闭实测可用（源码静读曾误判，行为探针纠偏）

- 结论（探针测试 apps/acp-console/tests/transport_semantics.rs 锁定）：agent 侧
  流级 `shutdown()` 经 Compat → futures 默认 `poll_shutdown` → **委托 `poll_close`**
  → yamux CloseStream 帧照发，对端读到干净 EOF。半关闭在本底座可用。
- 教训：源码静读「没看到 poll_shutdown 覆写」就断言 no-op 是错的——futures-io
  的 trait 默认实现本身就是委托链，行为结论必须以行为探针为准（本次探针
  一次运行即纠偏，省掉按错误假设改造 pump 的工作量）。

## F2 双向并发大消息传输存在随机停滞（kick 自愈，精确机制待上游确认）

- 实测：单条 200KB WS 消息在双向并发透传下 4/5 随机停滞 10s+，trace 停在
  最后一个 WindowUpdate 处理后全线静默；agent 侧 echo 4 读只完成 3。
- 疑点指向：`flow_control.rs next_window_update`（:44-54）批量策略——未消费满
  半窗（< max/2）→ 不发更新；`stream.rs poll_read`（:312-317）里窗口更新遇命令
  通道满返回 `Pending` 时**跳过且不排队重试**，仅等下次 `poll_read`——读者一旦
  空闲，credit 滞留，写侧停等。精确唤醒链需上游确认。
- 同族上游缺陷 rust-yamux#189（RecvClosed 不唤醒读者丢 EOF）已在 0.13.3 修复
  （本仓 pin 0.13.10 已含）；F2 属写侧 credit 滞留变体，截至 0.14.0 changelog
  未见处理。
- 消除实证：泵改「16KiB 分块写 + 每块 flush + 5s 超时重 kick」后 6/6 稳定且
  吞吐更好（0.07s vs 0.15s）——分块 flush 让 credit 消耗平滑，绕开批量阈值
  死锁窗；超时重 poll 兜住残余唤醒丢失。

## 消费方兜底（已落地 apps/acp-console/pump.rs，其他长流消费者照抄）

1. 双向泵 spawn + `select` 首侧结束即 abort 另一侧——`join!` 双侧都 pending 即永不结束。
2. 写分 16KiB 块（对齐 split_send_size）+ 每块 flush + 5s 超时重 kick；读 30s 超时重
   kick——超时即 WARN 留痕后重新 poll（重 poll 会补发被跳过的窗口更新，打破死锁环）。
3. 连接死亡以 `NodeEvent::PeerDisconnected` 与泵 select 竞速兜底（半关闭之外的
   真实死亡路径：进程死亡/网络断，EOF 不会出现，事件是唯一可观察信号）。

## 建议底座治理项（交 crates 域 owner 排期）

1. 向上游 rust-yamux 提「双向并发突发下窗口更新滞留致写侧饿死」issue（附本
   文件 F2 复现参数与 trace），并评估 0.14.x 升级（连接 ID 去 rand 等）。
2. 底座 README 增补：长流消费者的存活判据 = PeerDisconnected 事件 + 流级
   shutdown 的 FIN/EOF（后者已探针锁定可用），暂停滞类死等。
