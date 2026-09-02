# Known Issues

<!-- 格式：症状 → 原因 → 修法。排查超过 5 分钟的 bug 才值得记。 -->

_none yet — be the first._
## 2026-09-01 rustc 1.98 io::Error API 变化（downcast_ref 消失）
症状：`err.downcast_ref::<E>()` 报 E0599 "no method named downcast_ref"；`err.into_inner()` 返回 `Option<Box<dyn Error + Send + Sync>>` 而非裸 Box。
原因：本机工具链 rustc 1.98.0 (2026-08) 的 std::io::Error API 已演进，旧写法全部失效。
修法：`err.into_inner()` 先处理 None，再对 Box 用稳定的 `downcast::<E>()`（Result<Box<E>, Box<dyn Error>>）；封装成 flatten_io 一类的还原函数，测试与库共用。

## 2026-09-01 rustc 1.98 unused_mut 误报与 E0596 死锁
症状：`let (mut tx, mut rx) = duplex(..); write_frame(&mut tx, ..)` 报 unused_mut 警告，但按建议删掉 mut 立即报 E0596 cannot borrow as mutable——二者矛盾，-D warnings 下无解。
修法：保留 mut，在 let 语句前加 `#[allow(unused_mut)]` 并注释说明矛盾原因；不要全文件 allow。

## 2026-09-02 p2p 内核传输（Rust/cargo 生态）

- cargo fetch 拉不到清单未引用的 crate：quinn/yamux/snow/rcgen 只有写进 Cargo.toml 后 fetch 才下载。症状：registry/src 里 grep 不到版本目录。先改清单再 fetch。
- yamux 0.13（paritytech）无 Control/async API，纯 poll 模型（poll_new_outbound / poll_next_inbound）；连接必须被持续轮询才会冲刷流写缓冲。驱动任务等待开流请求时若阻塞在 mpsc.recv() 上，对端流写入会永久卡死——须用 tokio::select! 单点驱动（连接轮询与开流请求竞争）。
- quinn RecvStream::poll_read(cx, &mut [u8]) 会把整个传入切片注册为 ReadBuf，调用方（tokio AsyncRead 适配器）必须先用 ReadBuf::remaining() 截断长度，否则 put_slice 断言 panic（"buf.len() must fit in remaining()"）。
- snow write_message 输出缓冲必须容纳 token 开销：XX msg2 = e(32)+s(48)+tag(16)+payload，只留 payload+64 会得到 Err(Error::Input)（"snow: input error"），极易误判为解密失败。
- tokio-util 0.7 没有 copy_bidirectional；它在 tokio::io 下且签名是两条流（a->b 与 b->a），单流自回环要用 io::split + io::copy + shutdown。
- rustls 0.23.43：ClientConfig with_client_auth_cert / ServerConfig with_single_cert 收 PrivateKeyDer（由 provider 加载），不再收 Arc<dyn SigningKey>；danger 校验器在 client::danger / server::danger；quinn 的 conn.peer_identity() 返回 Box<dyn Any>，downcast 目标是 Vec<rustls::pki_types::CertificateDer>。

## 2026-09-02 p2p 中继穿透（Rust/cargo 生态）

- cargo clippy 不认 --message-format（cargo test 可以）：接在 -- 后报 "Unrecognized option: 'message-format'"，clippy-driver 参数路径不同。clippy 要短输出直接看默认格式。
- prost derive 手写 oneof 信封：字段属性 #[prost(oneof = "relay_msg::Kind", tags = "...")] 里的模块路径是字符串，必须与实际 pub mod relay_msg 路径逐字一致，写错只在 decode/编译期报隐晦错误。
- 只新增了 impl 块文件却忘在 lib.rs 声明 mod x;：文件不在模块树，报错是调用处 E0599 "method not found in Arc<T>"，离缺失点很远；新增文件先补 mod 声明。
- 本仓 thiserror 1.x/2.x 多版本并存，cargo 任何一次构建都会把锁文件成员依赖行 "thiserror" 规范化为 "thiserror 1.0.69"（多版本消歧），导致 git worktree remove 报 "contains modified files"。修法：git diff 确认仅此漂移后 checkout -- Cargo.lock 再 remove，勿直接 --force。
- prost 手写消息要求派生 prost::Oneof 的 enum 用 #[prost(message, tag = "n")] 标注每个变体，tags 列表要与 tag 集合一致，漏一个 tag 解码未知字段时静默跳过。

## 2026-09-02 X 构建门禁（bash / fmt 门禁上线）

- bash test 内建的 `[ "$s" = $pat ]` 对未加引号 RHS 不做通配匹配（通配只在 `[[ ==` 与 `case` 生效）。症状：LINE_LIMIT_EXEMPT 填了精确路径豁免仍 exit 1。修法：is_exempt 用 `case " $LIST " in *" $1 "*) return 0 ;;`；301 行探针拦截测试当场暴露此 bug。

## 2026-09-02 p2p-relay 控制流秒断与配额自锁（中继兜底不可用根因，E4-S 待修）

- 症状：CLI 节点 --relay 接线后 relay 会话 connect 成功但 control ~90ms 即断（客户端 "relay control closed; reconnecting"，服务端 "control read failed; cutting"），~0.5s 重连循环；数分钟后报 "relay rejected: code=3, per-peer circuit quota exceeded" 锁死；punch 信令必败（"punch signaling failed: connection lost"），降级链 Direct→Punch→Relay 走不通。
- 定性：干净态（bootstrap 刚重启 + 单一客户端）31/31 复现 ~90ms 断——与负载无关的必然缺陷，非偶发；代码定位 p2p-relay/src/control.rs、slots.rs、limits.rs。
- 配额自锁机理：每次 control 重连内含 reserve（slots.rs issue_circuit，TTL 最长 3600s）计入 per-peer 配额（limits.rs max_circuits_per_peer=32，DEFAULT_TTL_SECS=300）；churn 节奏 ~0.5-5s ⇒ 稳态负载 60>32 必然自锁，TTL 滚动恢复后再锁。
- 138 bootstrap 的 ufw 从未放行 3403/udp+3404/tcp（relay 客户端此前为零，缺陷与不可达均未暴露）。修 relay 层前，ECS 需每次冒烟前重启 p2p-bootstrap 换取 ~150s 健康窗口。

## 2026-09-02 无 --observation 的节点注册 loopback 地址（跨网不可拨）

- 症状：跨网 ping 报"目标未被发现"或拨 127.0.0.1；discover 显示对端地址全为 127.0.0.1:x。
- 原因：无观测器时注册地址=观测(空)×端口+监听地址，展示为 loopback（assembly merge_observed_with_listen）。
- 修法：公网节点一律带 --observation <公网IP>:3402；冒烟编排里把这一项列为起节点前置检查。
- 从未跑过 rustfmt 的存量仓库接 fmt 门禁：rustfmt 1.9 默认 style_edition 2024，首次 --check 就是 32 文件真实 diff，不是门禁 bug；且格式化拆行会让贴线文件越过行数红线（实测 284→315、281→314），fmt 与行数两条门禁连环爆。上线顺序应为：先摸底存量违规 → fmt 归一提交 → 立即复查行数 → 超线文件抽测试子模块。

## 2026-09-02 U 互操作测试（tokio duplex + MITM 转发管道，挂死 15 分钟）

- 症状：duplex 上用两个双向转发管道做 MITM 篡改握手，一侧按预期报错后另一侧永久挂起；sample 只见 runtime park 在 kevent、无任何注册 waker，任务全在等永远不来的数据。
- 原因：tokio::io::split 是 BiLock——对端看到 EOF 的条件是整条 DuplexStream 的两半（ReadHalf+WriteHalf）全部 drop；正向管道退出只归还了自己那两半，反向管道仍持有另一半对，对端永远等不到 EOF。
- 修法：正向管道退出时经 oneshot 通知反向管道，反向用 tokio::select! { 转发循环, 通知 } 竞争退出，两半同时归还后 EOF 才能传播。"管道对管道"拓扑必须配对退出，单向 EOF 传播不完备。
- 附：snow Noise XX 首帧（-> e）无密钥、不加密不验 MAC，篡改 msg1 当场不报错，失败延迟到 msg2 解密 MAC 不匹配才出现；写篡改测试时按这个时序预期错误出现的位置。

## 2026-09-02 p2p 安全修复轮（relay M2/M5/L3）

- 测试夹具 mock 服务端链路把 peer_id 标成 relay 自身（mock_link_pair(a, "relay")），违背 RelayLink 接缝契约（peer_id 须为对端身份）：服务端视角下所有客户端流塌缩成同一 peer。症状隐蔽——属主/配额校验加上前毫无异常，加上后表现为"校验形同虚设"或"停车方永久等 Bound 超时"。修法：夹具两侧都标客户端身份。p2p-itest 的 relay_pair 同病，一起修。
- if let Err(x) = self.lock().foo() { ...await... } 的临时 MutexGuard 活到 if-let 结束，跨 await 使 future !Send（std MutexGuard 非 Send），报错只说 "future cannot be sent" 不指认守卫。修法：先把结果 let 绑定收口锁临界区，再 if let。
- 仓库已在跑 cargo fmt 的前提下，凭记忆写 edit 的 old_string 必失配（fmt 会拆行/合行）。流程必须是：读当前文件 → edit；或先 cargo fmt 再批量 edit。
- git worktree remove 报 "contains modified files" 且 diff 仅 thiserror → thiserror 1.0.69 规范化漂移时，checkout -- Cargo.lock 后即可 remove（本仓多版本 thiserror 并存所致，见上期）。

## 2026-09-02 tokio::select! 守卫状态被分支 future 内部改写 -> 唤醒丢失
症状：yamux 驱动 select 的 open_rx 分支带 guard `pending_open.is_none()`，
而 poll_fn 分支的 future 内部 take/放回 pending_open——select 挂起决策基于
poll 时点快照，快照后守卫翻转不会重评，open_rx 分支被禁用且 waker 不注册，
后续请求唤醒永久丢失（空闲/连续第二次 open_stream 必挂）。
修法：跨 await 修改守卫状态的处理逻辑移出 select 分支 future，在 loop 顶部
独立处理；select 分支 future 保持只读。诊断手法：循环计数打印定位 driver
卡死轮次，再二分变量（次数 vs 时间）——"闲置后失效"未必与时间有关。
## 2026-09-02 多接口 mDNS 宣告 + 共享 LAN -> 冒烟 ping 间歇性全地址拨号失败
症状：p2p-cli 冒烟 discover/ping 时好时坏；失败轮 ping 报最后一个地址
Connection refused/No route to host，node1 日志见 tcp inbound handshake
failed: early eof（客户端侧超时中止）。
原因：facade mDNS 按全部本机接口宣告（含 fe80 链路本地无 %scope、240e 全局
不可达），共享 LAN 上其他 p2p 节点（并行会话/其他机器）同 namespace 互相
发现，地址簿膨胀到约 20 个死地址；拨号走查慢且 node accept 循环被并发入站
握手拖住，loopback 握手等超时被客户端掐断。
排查：保留现场（mktemp 目录不删）+ RUST_LOG=info 重跑失败轮，对比通过/失败
轮的 discovered peer 数量与地址集；sample 看进程线程栈排除假死。
修法：测试路径收窄发现面（--no-mdns 只走 rendezvous，地址簿仅 127.0.0.1）；
拨号侧多地址预算放大（REQUEST_TIMEOUT 5s 到 20s）。多接口死地址的真正治理
（scope zone、地址优先级、dial 并发竞速）属 facade/swarm 层，已报协调会话。

- 症状：tracing::event!(level_var, ...) 编译报 E0435 "non-constant value"（macro 内 static __CALLSITE 需要字面量级别）。原因：event! 动态级别分支对表达式 level 走不了静态 callsite。修法：落盘处用 if level == Level::WARN { warn! } else { debug! } 字面量宏分支，策略函数只做级别判定并返回级别供单测断言（2026-09-02 E4 实录）。
- 症状：tokio::pin!(x) 报 warning "variable does not need to be mutable"。原因：pin! 内部重新绑定，外层 let mut x 多余。修法：被 pin! 的绑定声明成 let x（无 mut）（2026-09-02 E4 实录）。

- 症状：edit 工具对 docs/coordination.md 报 old_string was not found，肉眼对照"完全一样"。原因：本仓库中文文档用全角标点（，：（）、），从对话/终端回显里抄的是半角替代。修法：先 grep -n 取目标行原字节，从输出原文复制 old_string；连续两次失配就该怀疑标点宽度（2026-09-02 协调表编辑两连败实录）。
- 症状：云机装 rustup 时 curl exit 35（Connection reset by peer）。原因：sh.rustup.rs 从大陆云机常被连接重置，走默认地址必翻车。修法：安装脚本也走镜像 https://rsproxy.cn/rustup-init.sh，配合 RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT=rsproxy（2026-09-02 ECS 部署实录，重跑前必改）。
- 症状：rendezvous 链路日志每 ~5s 报 "connection lost" 周期刷屏，疑似 bootstrap 故障。原因：这是全系统常态——链路生命周期 ~5s + 30s 退避重注册，注册表靠周期重注册维持；138 与 ECS 基线同节奏（coordinator→138 一晚 318 条同款 WARN）。修法：判定前先对照健康基线节奏，别当部署缺陷（2026-09-02 ECS 部署实录）。
- 症状：TCP 引导 /t3401 握手成功但会话即断（"read stream ended"，服务端同步断），同路径 QUIC 正常。原因：YamuxMux 语义=「全部句柄丢弃即关闭连接」（swarm 门禁/重复连接丢弃依赖，文档化设计），而 TransportLink::connect（facade p2p crate）open_rendezvous_stream 后 return stream_to_conn(stream) 丢 SecureConn，TCP 会话被自身关闭语义杀死；QUIC 的 quinn 连接由驱动任务持有不受影响。修法：持有 SecureConn（挂进 stream_to_conn 写任务闭包，连接随 RendezvousConn 丢弃收敛）；与公网分段/MTU 无关——整段无分段管道同样断（2026-09-02 E4 K 会话消融实录，回归见 p2p-itest/tests/tcp_wan_bootstrap.rs）。
