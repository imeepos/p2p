# E5 契约与遗留项处置记录

- 会话：E5（可观测性/长稳/安全加固轮），分支 feat/e5-observability-stability-security
- 依据：代码实测阅读 + coordination.md E5 候选登记 + ECS/E4 冒烟记录；
  「数据驱动再议」项按本轮实测/长稳方案给出裁定或登记口径

## 1. 重连退避复位语义（本轮裁定并落地）

- 原语义：relay 会话链路拨通即 `Backoff::reset()`。E4 churn 实测暴露：对端
  「连上即断」闪断时序列被反复归零，重连间隔钉死 base 值（500ms），紧密循环
  放大配额与日志压力（E4 修复 7b901ed 根治的是配额自锁，复位语义未动）。
- 裁定：复位以会话健康为前提——`Backoff::reset_if_healthy(started_at, min)`，
  relay 会话 min=10s（MIN_HEALTHY_SESSION）；首次建链直接复位保持原行为。
  参数取值依据：base 500ms、cap 30s 下，10s 健康窗足以覆盖「瞬断重试成功」
  与「真闪断」的区分；长稳期以 relay_reconnects 指标复核，若仍有钉死形态
  再上调（数据驱动）。
- 回归：backoff.rs `reset_if_healthy_requires_min_uptime`。

## 2. mux 生命周期契约（文档化，代码不变）

- YamuxMux（TCP 侧）：「全部句柄丢弃即断链」是设计语义而非缺陷——swarm 门禁
  拒绝、重复连接丢弃都依赖它。所有权规则：谁持有最后一个流句柄，谁负责连接
  存活；facade TransportLink 必须持有 SecureConn（E4 缺陷 0698963 根因，
  回归 crates/p2p-itest/tests/tcp_wan_bootstrap.rs）。
- QuicMux（QUIC 侧）：quinn 连接由传输层驱动任务持有，句柄丢弃不断链；
  mux 句柄仅是流的视图。两侧语义不同是既有事实，统一为长期方向（不改）。
- Swarm 契约：`serve_connection` 持 mux 至 accept_stream 返回 None 或关停；
  断链即出池并发 PeerDisconnected。已知限制：mux 层不冒泡断链原因，
  accept_stream None 无 reason 字段；断链归因依赖传输层日志与重连路径日志
  （E5 已保证重连告警携带原因），深挖原因需 MuxControl 契约扩展，登记缓办。

## 3. 观测多反射器（按实测登记缓办）

- 现状（observe.rs 实读）：`--observation` 已支持多值（clippy Append），
  但 `observe_external_addrs` 首个成功即 break——多反射器只做容错（第一个
  不通换下一个），不做交叉验证/合并，也无 v6 反射路径（反射器绑定 0.0.0.0，
  观测结果恒 v4 映射）。
- 实测依据：ECS 冒烟记录「UDP 空闲 12s 映射稳定不漂移」「观测取首个成功反射
  （恒 v4）」；单反射器在双公网拓扑下已满足注册可拨性。
- 裁定：缓办。解锁条件：长稳期出现 v4 映射漂移或单反射器故障切换失灵，
  再实现「多反射器结果交叉 + 映射一致性校验」；否则维持现状（避免为冗余
  而引入择优歧义）。

## 4. IPv6 支持（按实测登记缓办）

- 已有：地址模型与排序支持 v6（book v6 /64 同前缀判定、is_global 分支、
  单测含 240e: 地址）；观测/展示层可携带 v6 字符串。
- 缺口（实读确认）：全栈监听绑定 0.0.0.0（Swarm::start、observe 反射器、
  relay accept），quinn client endpoint 亦绑 IPv4 未指地址——出站 QUIC 拨
  v6 地址不可达；E3 记录的「直连 v6 11.9ms」为 LAN 内经 mDNS 学到 v6 地址
  的同网段 TCP 通路，跨网 v6 端到端从未验证。
- 裁定：缓办（E6 候选）。解锁路径：bind dual-stack（:: + IPV6_V6ONLY=0）
  一处改动覆盖 QUIC/TCP/观测三面 + 真实 v6 环境验证；当前所有验收场景
  v4 通路闭环，v6 属能力扩展而非缺陷。

## 5. 其余本轮相关裁定

- 错误原因链：RelayEvent::ControlClosed 携带 reason（EOF/io 错误原文），
  会话重连告警带 reason；降级链末次错误聚合 direct/relay 原因（E5 提交
  9b4bf65）。mux 断链原因冒泡为唯一未覆盖面（见 §2 限制）。
- 活跃电路数口径：客户端侧以「中继跳成功的在池连接」近似（active_connections
  与 relay_sessions_active）；服务端真值在 RelayMetricsSnapshot.circuits_active
  （bootstrap 指标日志），两端口径在 docs/notes/e5-soak-report.md 说明。
