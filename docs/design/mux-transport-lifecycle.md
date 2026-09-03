# Mux 与传输层连接生命周期语义：YamuxMux / QuicMux 对照与统一定稿

状态：E8-H3 定稿（2026-09-03）。依据：coordination.md 检查轮 25 复盘（TCP 会话自毁）、
E4 裁决方案 A（挂闭包保生命周期）、docs/research/p2p-connection-lifecycle-survey.md 第 8 章。
本文每条事实写前对照代码核验（引用只到 文件+符号，行号会漂移），与早期设计稿无承袭关系。

## 1. 背景

检查轮 25 定位的自毁缺陷：facade TransportLink::connect 曾把 SecureConn 提前丢弃，
TCP 侧 YamuxMux 触发 close-on-drop 立即自毁；QUIC 侧因 quinn 端点驱动任务独立持有
连接而幸免。同一句柄使用方式在两条传输上行为相反，且无任何契约文字说明——登记为
契约缺口「YamuxMux/QuicMux 生命周期语义统一或文档化」（E5 候选转 E8-H3）。

## 2. 现状对照（四维度）

| 维度 | YamuxMux（TCP，yamux 0.13.10） | QuicMux（QUIC，quinn 0.11） | 一致性 |
|---|---|---|---|
| 句柄全部丢弃 | 连接终止（close-on-drop） | 连接存活，至多活到空闲超时或活跃流结束 | 不一致（自毁缺陷根源） |
| 显式 close() | 驱动任务退出，连接随传输层断开 | CONNECTION_CLOSE（code 0 hangup）立即通知对端并本地收敛 | 语义一致，机制不同 |
| 空闲行为 | 无任何死链判定（本轮起 SO_KEEPALIVE 30s/10s） | keepalive 10s + idle timeout 30s | 本轮对齐 |
| 读半结束 | 流级 EOF + 会话级 accept None；传输 EOF 即会话终止 | 流级 EOF + 会话级 accept None；QUIC 无连接级半关闭 | 语义一致 |

### 2.1 句柄全部丢弃

- YamuxMux：驱动任务 drive 独占 yamux::Connection（p2p-mux/src/yamux_mux.rs，
  YamuxMux::new 内 spawn）。最后一个句柄丢弃后 close_tx 归零，drive 的 select 首分支
  close_rx.recv() 返回 None 即退出，Connection 连同底层 TcpStream 一起 drop，对端以
  传输层 EOF 收敛。
- QuicMux：持有的 quinn::Connection 只是句柄，连接状态在 quinn 端点驱动任务内
  （p2p-mux/src/quic_mux.rs）。全部句柄丢弃后连接继续存活，由空闲超时
  （QUIC_IDLE_TIMEOUT，p2p-transport/src/quic.rs）回收，活跃流还能继续把连接钉住。
- 差异后果：上层无法用一个统一的「我丢句柄了」表达表达挂断意图。swarm 池只存
  conn.mux（p2p-swarm/src/swarm/dial.rs 与 listen.rs：Ok(conn.mux) /
  insert_connection(conn.mux)），mux 句柄即上层的存活锚，挂断一律走显式 close()
  （hangup.rs / lifecycle_handlers.rs 探活判死路径）。

### 2.2 显式关闭

- YamuxMux::close：容量 1 的 close_tx try_send 通知 drive 退出；Full（在途）与
  Closed（已退出）两态都被容忍，天然幂等。注意 close/drop 路径不发任何 yamux 层
  优雅帧：yamux 0.13.10 的 Connection 无 Drop 实现，其 close(self) 优雅状态机未被
  本 crate 调用，GoAway 仅在接收方向被解析为会话终止。
- QuicMux::close：conn.close(VarInt(0), b"hangup")，QUIC 层 CONNECTION_CLOSE 立即
  发出并本地收敛（在途操作即时报 LocallyClosed），对端 accept_bi 以错误收敛；
  quinn 首次调用生效，重复调用无副作用，幂等。
- 关闭后 open_stream 的错误形态：YamuxMux 因 open 通道对端消失得
  BrokenPipe("mux closed")；QuicMux 的 open_bi 以 quinn Closed 错误映射为
  ConnectionReset（错误链保真见 E7-K2）。kind 不同、都是显式失败，登记不修（见 5.b）。

### 2.3 空闲行为

- QUIC：客户端 keep_alive_interval = KEEP_ALIVE（10s），双侧 max_idle_timeout =
  QUIC_IDLE_TIMEOUT（30s），半开死链 30s 内显式回收（quic.rs）。
- TCP：yamux 0.13.10 的 Config 只有流窗口/流数等字段，无任何 keepalive 能力（crate
  源码核验；协议帧有 Ping/GoAway 但无主动发送 API），tokio TcpStream 缺省也不开
  SO_KEEPALIVE——半开死链无限滞留，直到下一次写失败。
- 本轮对齐：p2p-transport/src/tcp.rs 的 enable_keepalive 在 dial/accept 两条建流
  路径统一开启 SO_KEEPALIVE，参数复用 quic.rs 常量（起探 30s、间隔 10s）。
  keepalive 探测不产生应用层可读字节，yamux 驱动循环与上层「活跃」判定不受扰动。

### 2.4 读半结束

- 流级：对端 FIN。yamux 流读出 0 字节；QuicStream::poll_read 把 quinn 的 0 字节读
  映射为「零填充的 Ok(())」（tokio EOF 约定，quic_mux.rs）。一致。
- 会话级：TCP 传输 EOF 被 yamux 视为会话结束（drive 的 Flow::Closed 分支），
  inbound 通道关闭，accept_stream 返回 None；QUIC 没有连接级半关闭概念，对端
  close/空闲超时令 accept_bi 报错，QuicMux 记 debug 后返回 None。API 语义一致。
- TCP 特有：对端只关写半（shutdown(WR)）即触发上述会话终止——yamux 会话与传输
  同生共死；QUIC 单向流关闭不影响连接。上层语义按「会话级 None = 连接不可再用」
  理解即可，两实现都满足。

## 3. 统一语义定稿

1. 连接终止只认三类事件：本端显式 close()、对端关闭、传输层错误/空闲超时。
   close() 是唯一显式终止入口，幂等、已关闭连接上无副作用（MuxControl trait
   rustdoc 已同步写入契约面）。
2. 句柄存亡不作为关闭语义：过渡期允许 2.1 的实现差异存在，但上层规则是——
   把 mux 句柄当存活锚持有到用完，挂断必须显式 close()；既禁止「丢弃即关闭」
   依赖（E4 自毁教训，TCP 侧立即生效），也禁止「丢弃仍存活」依赖（QUIC 侧
   30s 后仍会被空闲回收，属慢性错）。
3. 空闲死链判定是传输层职责且两路对齐：QUIC 10s/30s，TCP SO_KEEPALIVE 30s/10s
   （本轮落地）。应用层若需更快的判死（秒级保活），按调研第 8 章建议 3 走
   协议层心跳，不依赖本层。
4. 读半结束两级语义（流级 EOF、会话级 accept None）为契约要求，两实现已一致。

## 4. 本轮代码对齐（已实施，全部加法路径）

| 对齐项 | 位置 | 说明 |
|---|---|---|
| TCP SO_KEEPALIVE | crates/p2p-transport/src/tcp.rs enable_keepalive（dial/accept 调用） | 参数复用 quic.rs 的 QUIC_IDLE_TIMEOUT/KEEP_ALIVE；设置失败 WARN 不阻断建连 |
| 契约面文档化 | crates/p2p-mux/src/lib.rs MuxControl rustdoc | 统一语义写进 trait 契约；trait 签名与类型形状未动 |
| facade panic 清零 | crates/p2p/src/rendezvous.rs、assembly.rs | 豁免收缩前置，见 scripts/check/panic-hygiene-exempt.txt 收缩记录 |

## 5. 只登记待裁决（本轮不实施）

a. 句柄存亡语义的最终统一（契约缺口本体）。候选方向：
   ① 统一 close-on-drop：给 QuicMux 加 Drop。会立即杀掉全部活跃流，牵动 relay 的
   流级桥接与 E8-S1 空闲回收的「使用中豁免」判定，须先完成上层持有审计；
   ② 统一「句柄不锚定存亡」（libp2p Swarm 模式）：YamuxMux 驱动任务脱离句柄
   存活，所有上层路径补显式 close，改造面大；
   ③ 维持现状 + 本文档（本轮采纳）。按「实测复现缺陷才修」原则，①② 待实测
   反证再议。
b. close() 后 open_stream 的 io kind 差异（BrokenPipe vs ConnectionReset）：E7-K2
   已裁决 kind 维持原逻辑，保持登记不改。
c. yamux 应用层 keepalive：0.13.10 无主动 ping 能力，分钟级以内空闲感知须自研
   帧或换实现；当前 30s OS 级判定够用，登记备忘。
d. Transport trait 无连接级 close 挂点：QuicTransport::close 是 endpoint 级关停
   （全部连接 + 停止 accept）。若未来需要按连接优雅关闭入口，走加法路径新增。

## 6. 上层现状核对（写前逐条核验，未做改动）

- p2p-swarm：池存 mux 句柄（dial.rs/listen.rs/relay_session.rs）；显式关闭三处
  （hangup.rs 用户挂断、lifecycle_handlers.rs 探活耗尽判死、dial.rs 竞态落选），
  均符合第 3 节规则 2。
- crates/p2p facade：rendezvous 链接以 stream_to_conn_owned 把 SecureConn 挂进
  写任务闭包持有到用完（E4 方案 A），符合规则 2。
- p2p-relay：全部工作在流级（crate 内无 SecureConn 引用），不锚定连接存亡，
  对句柄语义不敏感。

## 7. 自查记录

- 写前实现阅读清单：p2p-mux（lib/yamux_mux/quic_mux/limited）、p2p-transport
  （lib/quic/tcp）、p2p（rendezvous/assembly/node）、p2p-swarm（dial/listen/
  hangup/lifecycle_handlers/relay_session，只读）、p2p-relay（grep 审计，只读）、
  yamux 0.13.10 与 socket2 0.6.5 crate 源码（keepalive/GoAway/Drop 结论来源）。
- 表格逐行对应：2.1→drive select 与 quinn 驱动模型；2.2→两个 close() 函数体与
  yamux crate 无 Drop 事实；2.3→quic.rs 常量与 tcp.rs enable_keepalive；2.4→
  poll_read/accept 分支。本节与代码不符即文档缺陷，改代码或改文档须同步。
- 消融：panic 豁免收缩以门禁机械验证——收缩后清单使旧代码 gate exit 1
  （rendezvous.rs 两处 expect 被点名），清零修复后 exit 0；keepalive 无读回
  API，无机械消融，以 p2p-transport 既有 echo/tcp_timeout 回归兜底，失败路径
  留 "tcp set_keepalive failed" WARN 观测信号。
