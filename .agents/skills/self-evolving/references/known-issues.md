# Known Issues

<!-- 格式：症状 → 原因 → 修法。排查超过 5 分钟的 bug 才值得记。 -->

## 2026-09-04 tokio::select! 相对 sleep 被更快 interval tick 永久饿死
症状：rendezvous 客户端在死连接上以固定 20s 周期空转报错（write half closed）达数十分钟，永不重连；查询分支再也没执行过。
原因：connect_and_loop 的 select! 每轮循环重建相对 sleep（30s 查询档），20s 注册 tick 先到期就把 sleep 丢掉重建——查询分支的 30s 永远到不了期，错误永不传播。
修法：周期分支用绝对时刻（tokio::time::Instant + sleep_until），触发后再推算下一截止；链路级错误必须在当轮上抛触发重连，不能只记日志继续循环（rendezvous_facade_link itest + reconnect_tests 锚定）。

## 2026-09-04 裸传输链不自 accept 入站流被对端 liveness 掐线
症状：经 TransportLink 盲拨的连接约 33s 被 facade 对端判死关链（probe missed ×3），客户端只见 Link("connection lost")；E4 修复（持有 SecureConn）后仍复现。
原因：裸链不进 swarm，连接只被当作出站流用；对端 liveness probe 在该连接上开 ping 流，本端无人 mux.accept_stream，探活永不命中。
修法：TransportLink::connect 起 spawn_link_responder 循环，accept 入站流并 dispatch 内置 PingHandler；未注册协议 debug 关流（crates/p2p/src/rendezvous.rs）。E4「句柄持有」与本次「入站应答」是同一坑的上下半场。

## 2026-09-04 harness bash workdir 参数失效导致错树假绿
症状：run_code 的 bash 传了 workdir 指向 worktree，pwd 实际仍在会话主树；cargo test 测的是未修改的主树代码，全绿假象（本轮实录，修复前测试全绿但绿的是旧代码）。
原因：该 harness 会话 bash 固定以会话 cwd 运行，workdir 参数被忽略。
修法：命令内显式 cd 前缀 + 首行 pwd && git branch --show-current 自证；涉及树归属的结论（测试/提交）一律先看这两个输出。

_none yet — be the first.

## 2026-09-04 诊断清理改动后的 stale-read 与重复 locale 键
症状：连续 edit 同一批文件时出现 file changed since it was read，或全量测试报 locale object duplicate key。
原因：前一步工具已修改文件但本轮仍使用旧读取快照；批量插入 locale 时没有先确认键是否已经存在。
修法：每次 edit 失败或前一步可能改过文件，立即重新 read；插入翻译键前先 grep 目标键并用 git diff 检查重复。_
## 2026-09-03 E8 itest 裸流直写帧被对端拒识成 EOF
症状：itest 里 `Swarm::open_stream` 拿流直接 write_frame/read_frame，对端读侧报 ProtocolViolation，本端 read 得 early EOF，误判成连接层断链。
原因：open_stream 交付裸流，协议 ID 首帧由调用方写（design §5.4 契约）；不写协议 ID 就发业务帧，对端 dispatch 把业务帧当协议 ID 拒识。
修法：裸流先过 `p2p_protocol::open_with_protocol(raw, &id)` 再读写帧；echo/探针类助手统一封这一步。
## 2026-09-03 E8 快速回收配置淹没 Error 档断链归因测试
症状：conn_reclaim 的 Error 档用例 5s 内收不到 ConnectionClosed{Error}，默认配置节点却能收到。
原因：被测节点挂了阈值 1s 的快速回收，对端 shutdown 的拆链传播（quinn 关闭握手）到达前，空闲回收已抢先出池连接，serve 的 remove_if_same 不中，Error 路径被 Idle 路径顶掉。
修法：断链归因类用例一律用默认（慢）回收配置；测「拆链传播」语义时不同时注入快速回收变量。
## 2026-09-03 E8 cargo fmt 改盘后 write 工具拒绝覆盖
症状：跑过 cargo fmt 后再用 write 工具整文件重写，报 file changed since it was read。
原因：fmt 修改了磁盘文件，工具读写一致性校验以最近一次 read 为基线。
修法：任何会改盘的命令（fmt/checksum/生成器）之后，写前必须重新 read。
## 2026-09-03 版本 bump 只在 tag 首次暴露测试失败
症状：版本 bump 直推 main 后没有失败反馈，直到推 client-v tag 才在发布流水线看到 hardcoded version 断言失败。
原因：实际远端是 GitHub，但 GUI workflow 只有 tag/PR 触发；预留的 Gitea workflow 未执行，且无本地 hook/主线 push 全量 CI。
修法：GitHub 主线 push 与 PR 均执行全量 make check；GUI tag workflow 单独执行 GUI gate 并校验 tag commit 在 origin/main 历史内；发布前用 version-check/release-check 和不自动 push 的 release.sh。
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
- 纠正（2026-09-02 R-E4 relay 诊断）：此前将 rendezvous 每约 5s 的 "connection lost" 视为正常生命周期基线是不完整结论；p2p-transport/src/quic.rs 两处把 30s Duration 用 as_secs() 转成 quinn VarInt，实际单位为毫秒，导致 QUIC 空闲约 30ms 即 TimedOut。该误用与 relay 控制流秒断、rendezvous 重连刷屏、打洞信令丢失同源；修复为 IdleTimeout::try_from(Duration)，不应继续把该 WARN 当作健康基线。
- 症状：多树并行的构建/冒烟脚本跑了半天 "Finished" 但行为没变。原因：脚本里 cargo build 没 cd 到目标树，在主树构建、却执行另一棵树的旧二进制（2026-09-02 R-E4 smoke3 实录，配额修复被误判未生效）。修法：脚本内显式 cd 到目标树根，构建后 ls -la 产物核对 mtime 再启动；验证结论锚定产物版本而非命令成功输出。
- 症状：mock 链路上客户端进程已 drop，服务端的"控制流关闭"回调（流级 EOF）永远不来，泄漏类修复在 itest 复现不出。原因：RelayClient 读半被 read_ctrl_loop 任务钉住，tokio::io::split 是 BiLock，两半全 drop 对端才见 EOF——连接活着时流级信号不可靠。修法：服务端记账对"对端消失"用链路归零做兜底触发器（link 计数归零即回收），不能单押流级 EOF（2026-09-02 R-E4 lifecycle 双触发器设计动因）。

- 2026-09-02 E5：`local a=$(date +%s) b=$((a+60))` 同一 local 语句内，后项的算术展开先于赋值执行，`set -u` 下直接 unbound variable 炸出整个函数（soak 编排器实录，cleanup 被 trap 连带触发）。修法：每个依赖变量独立 `local x; x=...` 声明赋值两步走；含 trap cleanup 的脚本尤其要防中途退出。
- 2026-09-02 E5：想 dry-run 一个「无 --dry-run 参数、载入即 main」的编排脚本时，`bash -c 'source script.sh'` 会真实执行 main（远端节点被真实拉起）。教训：编排脚本从第一版就内置 --dry-run/--self-check 门；任何「测试性执行」前先 `grep -n '^\s*main'` 确认入口守卫。
- 2026-09-02 G-A：Tauri 2 即使 `bundle.icon: []` 留空，`tauri::generate_context!` 仍按默认路径找 `src-tauri/icons/icon.png`，缺失直接编译失败（failed to open icon），clippy/test 门禁全被卡死。修法：放一个最小合法 PNG（python3 struct+zlib 手写 32x32 仅 104 字节）即可解锁；官方文档只说"图标可留空"未提此强制项。
- 2026-09-02 G-A：`git apply --cached` 分 hunk 提交时，pathspec/patch 路径相对**当前 cwd**——在仓库根跑 `git diff -- src/types.rs`（文件实际在 apps/gui/src-tauri/src/）静默得到空 diff，解析 0 hunks。修法：diff 与 apply 前先核对 cwd 与路径前缀；hunk 数为 0 直接 fail，不许继续。

## pnpm 11 ignored builds 使 install 退出码 1（2026-09-02，gui-shell）

- 症状：pnpm install 报 ERR_PNPM_IGNORED_BUILDS 且退出码 1；pnpm run/exec 因
  verify-deps-before-run 触发 install 连带失败，表象像构建坏了。
- 原因：pnpm 11 默认拒绝含 postinstall 的依赖（esbuild），且把 pending 审批当致命错误。
- 修法：pnpm approve-builds <包名>（非交互，写入 pnpm-workspace.yaml 的 allowBuilds），
  之后 install 恢复退出码 0。项目级 package.json 的 pnpm.onlyBuiltDependencies 在
  workspace 下不生效，只有根配置有效。

## pnpm 11 ignored builds 使 install 退出码 1（2026-09-02，gui-shell）

- 症状：pnpm install 报 ERR_PNPM_IGNORED_BUILDS 且退出码 1；pnpm run/exec 因
  verify-deps-before-run 触发 install 连带失败，表象像构建坏了。
- 原因：pnpm 11 默认拒绝含 postinstall 的依赖（esbuild），且把 pending 审批当致命错误。
- 修法：pnpm approve-builds <包名>（非交互，写入 pnpm-workspace.yaml 的 allowBuilds），
  之后 install 恢复退出码 0。项目级 package.json 的 pnpm.onlyBuiltDependencies 在
  workspace 下不生效，只有根配置有效。
- 症状：pnpm build 报 TS1002 Unterminated string literal，源码里字符串字面量被劈成两行。
- 原因：run_code 写文件时模板字符串内容里的转义序列（换行符写法）被解释成真实换行写进目标文件。
- 修法：文件内容里的转义序列双写反斜杠；写完对生成文件抽查含转义的行。
- 症状：i18n locale 独立小提交单独构建时 tsc 报 t("xxx") key 不存在。
- 原因：视图波删了占位期 key，但旧挂载页还没删，中间提交不可独立构建。
- 修法：i18n 独立小提交只做加法；删 key 与删消费者同提交，或保留死 key 交给打磨波清理。
- 症状：i18next 严格类型 t 报 t(I18nKey, Record<string,string>) 不匹配任何重载（值被对到 defaultValue: string）。
- 原因：CustomTypeOptions 生成逐 key 签名后，「动态 key + 通用 values」组合无法在联合 key 上分配。
- 修法：收口一个 LooseT = (key, values?) => string 的松散签名做 as 转换（运行时与 t 等价），模板化摘要场景集中走它，普通场景仍用严格 t（2026-09-02 gui-views-monitor 实录）。
- 症状：Tauri/React 窗口全白无任何 UI，构建与全部测试绿。
- 原因：渲染期 ReferenceError 或 useSyncExternalStore 快照不稳定（selector 每次返回新数组）导致无限重渲整树崩溃；vitest 独立 config 不继承 vite 的 define，build-time 注入量（__APP_VERSION__）在测试环境是裸标识符。
- 修法：selector 按源引用 memo 或消费侧 useShallow；vitest.config 与 vite.config 的 define 对齐；jsdom 启动冒烟锁整应用可渲染（src/test/app-boot.test.tsx 先例：vi.stubEnv 后动态 import main，waitFor main 元素，断言无兜底文案）（2026-09-03 实录）。


## 2026-09-02 W6-S2 反馈打磨轮

- 症状：run_code 里用模板串写含 markdown 反引号的文档 → 语法错误 `Expected ',', got 'ident'`。原因：内容里的反引号终止了 JS 模板串。修法：内容改为字符串数组 `join("\n")`，或转义所有反引号。
- 症状：`git add ... && git commit -F - <<'MSG' ... MSG && git add ...` 链式 heredoc 只执行了第一段。修法：heredoc 提交逐条单独跑，不进 && 链；每次 commit 后 `git log` 核对落盘。
- 症状：改共享函数签名（toastError 二参 string → options 对象）后，边界外的 W6-S1 文件编译报错。修法：共享 API 变更一律带兼容层（`string | Options` 归一化），不越界改他人文件，回报中标注。

## 2026-09-03 gui-client CI 打包轮

- 症状：Windows job 的 Tauri 打包步骤 exit 0，upload-artifact 报 No files were found（nsis/*.exe、msi/*.msi 均无）。
- 原因：tauri.conf.json 写死 bundle.targets [app, dmg]，均为 macOS 专属格式；Windows 按此配置打包产出为零——构建成功不等于有安装包。
- 修法：targets 改 "all"（各平台出全量原生产物）；平台差异（Linux 只要 appimage/deb）在 workflow matrix 里用 --bundles 显式覆盖。
- 症状：matrix 里 macos-13 job 排队几十分钟零 step（API 里 runner_name 为空），随后整个 run 被 cancel，看似随机卡死。
- 原因：macos-13 runner 镜像 2025-12-04 退役（官方 changelog 2025-09-19），job 永远排不到机器；同时 workflow 标签重推触发 concurrency cancel-in-progress，把上一个还在跑的 run 连带取消，形成「重推→互杀」循环。
- 修法：换 macos-15-intel（Intel 末班镜像）；发版流水线跑着的时候不要重推同一标签。

## 2026-09-02 W6-S1 默认值打磨轮

- 症状：用户反馈表单「看不到加载值/恢复值」，但保存功能正常、全部测试绿；jsdom 控制台刷 Function components cannot be given refs。
- 原因：components/ui/input.tsx 是普通函数组件（React 18 不转发 ref），react-hook-form register 的 ref 丢失后 RHF 只能改 _formValues 无法改 DOM；reset/setValue 后表单状态与输入框显示永久分裂（2026-09-02 W6-S1 探针实测：reset observationPort=3402 后 DOM value 仍 ""）。
- 修法：受控化（useWatch 读 + setValue 写，PortField 先例）可绕过；根治是给 Input 包 forwardRef（跨 settings 边界，需单独派单）。判定"值 vs 显示"分裂用临时 probe.test 断言 input.value，跑完即删。

## 2026-09-02 W6-S3：零依赖 CDP 客户端三连坑（症状→原因→修法）
- 症状：驱动脚本无任何输出挂死。原因：自写 send() 只登记 pending 漏了 ws.send 真正发帧，Chrome 从未收到指令。修法：协议客户端先验证指令确实发出（最小 probe：/json/new + Runtime.evaluate 1+1），再怀疑对端。
- 症状：Page.navigate 后 loadEventFired 等不到。原因：hash-only 变更是同文档导航不触发 load 事件；/json/new 的 url 参数也不保证真的导航（target 常停在 about:blank）。修法：每格用唯一 query 强制文档级导航 + 轮询 location.href 到目标 origin。
- 症状：Runtime.evaluate 抛 SecurityError: localStorage access denied。原因：evaluate 落在 about:blank 上下文（上一条的后果）。修法：先确认 origin 再碰 localStorage。

## 2026-09-03 W7-G-U2 更新提醒前端轮

- 症状：edit 工具编辑 400+ 行 locale 文件后，回显的 after 全文里 relay 段看似出现重复块+内容变异，疑似文件损坏。原因：巨量 before/after 回显经 spill 截断渲染产生的显示伪影，不是磁盘真实状态。修法：大文件编辑后一律 git diff 权威核验（本次 diff 干净：恰好 +33 行），不要凭回显判断、更不要在惊慌中回滚。

## 2026-09-03 relay 页白屏：useFormContext 解构 null（react-hook-form v7）
- 症状：一进 relay 页整树落 ErrorBoundary（"界面出错了 / Something went wrong"），TypeError: Cannot destructure property 'control' of null；设置页用同一组件却正常，95 个测试全绿。
- 原因：RHF v7 的 FormContext 默认值是 null，useFormContext() 在 FormProvider 外返回 null，而其 TS 类型签名声称非空（编译器零告警）；RelayConfigCard 的 FormProvider 只包住 AddressListEditor，FactoryDefaultsNotice 挂在 Provider 外。设置页不炸是因为 settings-view 把全部卡包在顶层 Provider 内——context 缺失按「调用点组合」发生，单组合的测试看不见。
- 修法：Provider 必须包住该表单全部 context 消费者（useFormContext/useWatch/useFieldArray 同理）；组件测试按真实页面组合挂载（不带外部 provider），修复前必红；启动冒烟升级为逐路由导航断言无 ErrorBoundary 兜底。

## 2026-09-03 GUI 邻居列表大量 127.0.0.1 条目（rendezvous 全 loopback 豁免泄漏）
- 症状：客户端邻居表出现成堆 `127.0.0.1/u<随机端口>` 且永远离线的条目，用户以为被异常节点围攻/怀疑是自己。
- 原因：三层叠加。① 每个 data 目录一个身份 + quic_port 默认 0 临时端口，本机多实例（GUI/coordinator/maca/itest）互发现各成条目；② a9be8e2 的 filter_loopback 带「全 loopback 集合保持原样」豁免（为同机可发现性），观测失败（无 --observation 或 UDP 3402 被墙）的节点把 127.0.0.1 监听地址注册进公共 rendezvous（43.240.223.138/u3400，namespace p2p-base）；③ rendezvous 查询侧只过滤自身 PeerId，不过滤 loopback/私网，他人的泄漏条目全员可见；GUI 表格按 lastSeen 留历史，退出实例堆成「离线 · N分钟前」。
- 修法方向（未实施）：rendezvous 服务端拒收 loopback/link-local 注册；客户端查询结果过滤私网；观测失败只注册 relay 地址并留告警日志；GUI 固定 quic_port 减少地址碎片。
- 落地更正（同日晚，086c55b..5791e4e）：查询过滤已实施但必须以信任域为界——rendezvous 本体在同机（bootstrap 全 loopback）时关闭，否则误伤 a9be8e2 保留的同机可发现性（observe_addr 集成回归实证）；服务端 public_only 整单拒收（签名记录不可部分剥离）；观测失败/不可路由注册启动 WARN。完整生效还需 138/ECS bootstrap 重部署换装新二进制；GUI 固定 quic_port 砍去不做（多实例同机合法场景会撞端口）。

## 2026-09-03 拨通即闪断：发现过期谎报 + 双向拨号分家（GUI 节点列表点拨号）
- 症状：节点列表点「拨号」提示已连接，行内状态立刻翻回离线；反复重拨同一模式（用户主诉「还是有问题」）。
- 原因1：发现条目 TTL 过期（mDNS 15s / rendezvous 缓存 60s）在 forward_discovery 里无条件映射成 PeerDisconnected，哪怕连接池里活连接还在——发现面失联被渲染成连接面断开。
- 原因2：两端各拨一次产生两条连接，insert 先到者优先且败者静默 drop：QUIC 最后一个句柄 drop 即关链，对端刚拨通的连接秒死；两侧各留各的还让流与 serve 循环分家，单方向 request 永远无应答，且此后每次重拨都撞 duplicate 拒收——闪断的持久来源。
- 修法：on_peer_expired 先查连接池再决定发不发断开；insert_connection 按「恒保留较小 PeerId 一端拨出的连接」本地收敛（两端对每条连接的方向认知相反、结论一致，无需协商）；PeerDisconnected 只在 remove_if_same 真出池时发，挂断/关停改为主动补发。回归：p2p-itest/connection_liveness 四条。

## 2026-09-03 DSH 协调派发轮（devloop_ledger schema 与无参工具调用）
- 症状：devloop_ledger save 连环报账本校验失败（version expected 1 / updatedAt 非ISO / tasks[].goal、priority、status 缺失或取值非法）。原因：账本 schema 固定校验，不接受自由命名字段。修法：顶层带 version:1 与 ISO updatedAt；每条任务必带 goal（非空一句话）、priority（P0/P1/P2）、status（todo/doing/review/done/debt）、acceptanceCommand；title/scope/branch 等自由字段可并存。
- 症状：run_code 里无参调用 tools.session_link_list() 报 binding arguments must be lossless JSON。原因：undefined 不是合法 JSON 参数。修法：无参工具也要显式传 {}。
- 经验：并行派单任务书只写需求、范围红线与可机械执行的验收命令（用固定新测试文件名让验收命令确定性成立），不贴源码；三单范围互斥（docs / p2p-swarm / p2p-relay）才敢真并行，派单前先核对分支全合并、worktree 干净。

## 2026-09-03 std io::Error::source() 盲视载荷：错误链保真必须加包装器（E7-K2）
- 症状：`io::Error::new(kind, inner_error)` 后断言 `err.source()` 能拿到 inner_error，实测 source() 为 None，白盒用例当场红。
- 原因：std 的 `io::Error::source()` 返回「载荷自身的 source」而非载荷本身；载荷只能经 get_ref()/downcast_ref() 取到。直接装箱内层错误，source() 遍历对内层盲视，「沿 source() 还原内层」的验收形同虚设。
- 修法：薄包装器 `ChainedPayload<E>{inner}` 作载荷——Display 委托内层（err.to_string() 即内层原文），Error::source() 返回 Some(&inner)（遍历可达、可 downcast 还原类型与文案）。见 p2p-mux/src/lib.rs、p2p-transport/src/lib.rs。
- 同场加映：`Result::expect_err` 要求 Ok 值实现 Debug——SecureConn/BoxedStream 都没有；测试断言一律写 match 取 Err 臂（Err(e) => e, Ok(_) => panic!(...)），不用 expect_err。

## 2026-09-03 E6-R3 三则编译/工具陷阱（当场红，改法已验证）
- 症状：tokio::time::timeout(..).await.map_err(|_| { inner.pending.lock().await = None; .. }) 编译 E0728。原因：await 不允许出现在非 async 闭包内。修法：拆成 match + 早返回，清槽逻辑放在 Err(_) 臂里顺序 await。
- 症状：std::sync::MutexGuard 出现「future is not Send」，service 的 tokio::spawn 拒编译。原因：if let Some(x) = self.lock_state().retire(..) { ..await.. } 的 guard 临时值活到 if-let 结束，横跨 await。修法：先 let x = self.lock_state()...; 语句收口再 if let。
- 症状：write 工具对 worktree 内文件报 cannot overwrite without reading——主树同 commit 文件读过不算数，路径不同缓存独立。修法：对 worktree 路径先 read 再 write/edit；bash 验证同理必须显式传 workdir（默认 cwd 是主树，grep 会在错误目录出假阴性）。
## 2026-09-03 E6 swarm 新增事件变体打断既有 itest（hairpin_fastfail）
- 症状：swarm 在 NodeEvent 加三个生命周期变体后，cargo test 全过但 make check 的 hairpin_fastfail 红："expected PeerConnected after lan dial, got PeerStateChanged"。cargo test -p p2p-itest --test peer_lifecycle（新用例）全绿——机械验收命令跑到 make check 才暴露。
- 原因：该用例在 connect() 后只 recv 一个事件并 match 要求必须是 PeerConnected；监督者处理 DialStart 异步先于拨号完成，新变体排到了 PeerConnected 前面。广播流的「加法」对严格 match 消费方是行为变更。
- 修法：生命周期事件改走独立 broadcast 通道（Swarm::subscribe_lifecycle，任务书允许的「等价事件机制」），NodeEvent 冻结流零扰动，既有消费方零改动；新用例断言全部迁到新通道。
- 教训：给共享事件流加变体前，先 grep 全部 recv 点的 match 严格度；「验收命令只点名新测试文件」不等于「只有新测试会受影响」，make check 全量才是真相。

## devloop_accept 内置超时短于全量门禁时长（2026-09-03 E7 轮）
- 症状：验收命令 `cargo test … && make check` 经 devloop_accept 跑，几十秒后被杀，exitCode=null/verdict=fail，输出停在编译或测试刚起步——代码明明是绿的。
- 原因：工具内置超时不可配，短于本仓 make check（编译+全测试+GUI vitest 约 2-4 分钟）实际时长；超时被杀记为 fail，是假红不是回归。
- 修法：同一验收命令用 bash 后台任务长超时复跑，取真实 exit code 作判决并在账本/协调表备注「accept 工具超时，人工同命令复跑」；验收命令能拆出即时项（grep/test -f）的先单独跑掉。
- 症状：make check line-limit 报 mod.rs 301 行，手工把 if 块压成单行省 2 行后，cargo fmt 又展开回多行——压行数压在 fmt 会重排的语句上等于白压。
- 原因：rustfmt 会无条件展开非空单行 if/struct 字面量（3 字段起必拆多行），只对注释有保留。
- 修法：行数预算吃紧时压缩注释/拆文件（types.rs 拆 node_event.rs 先例），fmt 敏感语句上的压缩全部无效。

## tokio 没有 TCP keepalive 参数化 API（2026-09-03 E8-H3）
- 症状：凭直觉写 TcpStream::set_tcp_keepalive / tokio::net::tcp::TcpKeepalive，E0599 方法不存在；该 API 在 tokio 里从未有过（属于 socket2）。
- 原因：tokio 只有 TcpSocket::set_keepalive(bool)（OS 缺省参数），且在拨号前构造 socket 的类型上，accept 出来的 TcpStream 没有对应方法。
- 修法：workspace.dependencies 追加 socket2（已在 tokio 传递图内，锁文件只加一条直接边），SockRef::from(&TcpStream) 借用设参；with_interval 平台门逐家核过（macOS 有，with_retries 需 all feature），编译期即暴露不兼容。

## 编辑工具报 file changed since read：cargo fmt 是隐形第三方写手（2026-09-04 负载选路轮）
- 症状：edit 报 "file changed since it was read — re-read the file"，且报错发生在与目标文件无关的门禁运行之后。
- 原因：同批次里先跑了 cargo fmt（make 的 fmt 或脚本），rustfmt 重写了待编辑文件，编辑工具按读取快照校验失败。
- 修法：写→fmt→再改 的批次里，fmt 之后的编辑前必须重新 read 目标区域；能改成 写→读→fmt 顺序的先改顺序。

## tauri dev 秒挂 Port 5173 already in use：遗留 dev 会话占口（2026-09-04 诊断轮）
- 症状：pnpm run tauri dev 立刻 exit 1，beforeDevCommand 报 Error: Port 5173 is already in use；同时 GUI 持久化日志 frontend.log 停在数小时前的 [vite] Failed to reload 洪泛，窗口疑似僵死。
- 原因：tauri.conf.json devUrl 固定 5173，vite 绑不上端口直接带崩 beforeDevCommand；上一份 tauri dev（pnpm→tauri CLI→vite→p2p-console 四层进程）从未退出挂在旧终端。frontend.log 的 HMR 失败是旧 vite 实例僵死所致，当前源码 tsc -b 全绿，不是代码问题。
- 修法：lsof -nP -iTCP:5173 -sTCP:LISTEN 定位占用者，ps -o pid,tty,lstart,command 看进程树起点判定归属，kill 整树后重跑；重跑前 pnpm exec tsc -b 区分「旧会话僵死」与「真语法错」。

## run_code worker 瞬时坏状态：所有大参数调用报 "missing required property description"（2026-09-04 方案落档轮，耗时 10 分钟的误归因）
- 症状：write/bash 传较大内容（4-14KB）的调用连环报 invalid arguments: missing required property "description"，报错指向参数校验；同批小调用正常。连报 6 次，期间换了 5 种写法（占位符反引号、分块 heredoc、单行、base64）全部失败，逐特征二分（反斜杠/换行/中文/体积）全部无法稳定复现。
- 原因：code-runtime worker 进入瞬时坏状态，对超阈值参数的拒绝统一误报成 description 缺失；约 10 分钟后自愈，逐字重跑当初失败的程序全部通过——与参数内容零关联。
- 修法：同一错误在不同参数形状（不同工具/不同内容/带不带某字段）下持续出现时，先怀疑环境瞬时故障而非参数归因；用最小探针（纯 ASCII echo）+ 逐字重跑原失败调用验证恢复，再决定是否重构写法。探针通过后直接重试原操作，别为幻觉规律改写方案。

## 已修复的崩溃仍出现在持久化日志：dev webview 模块图分代（2026-09-04 日志复核轮）
- 症状：frontend.log 里 60 条 TypeError: Cannot destructure property 'control'（FactoryDefaultsNotice@factory-defaults-notice.tsx），时间戳晚于修复提交 6 小时，且栈行号偏移显示跑的就是修复后代码——看似「修复无效」。
- 原因：长命 tauri dev 会话的 webview 活过了整天的 HMR/依赖重优化，页面里 react-hook-form 存在两个生成实例：FormProvider 写入的 context 与 useFormContext 读到的 context 不是同一个对象（后者默认 null），解构即炸；同时段 [vite] Importing a module script failed 洪泛是同一模块图分代的旁证。磁盘代码与测试全绿，纯属 dev 会话运行态腐化。
- 修法：判定顺序——先 git log 确认修复提交早于报错时间，再核对栈行号偏移（react-refresh 注入约 +4 行）确认跑的是新代码，然后跑 vitest 回归（relay-config-card/settings-defaults/app-boot 路由冒烟）机械证明代码侧已修；结论落到「杀旧 dev 会话重启 webview」，不要回头改已经修好的代码。
## 2026-09-04 relay 控制流被服务端静默窗口精确切断：客户端周期不得与服务端超时同值贴线（线上重连风暴日志分析）
- 症状：两台独立 relay 的控制流都在 connected 后恰好 +10.008s / +10.034s 被 control stream eof (clean close by peer) 切断，周期恒定 10.5s 无限重连；客户端 keepalive interval=10s 的首次 KeepAlive 与服务端静默窗口同刻竞速，服务端计时器随 Reserve 处理启动、恒早 RTT/2，客户端永远输。
- 原因：本仓默认 server_silence=45s 且正常应答 KeepAlive（keepalive.rs 默认值、control.rs 控制循环），线上 relay 却按 10s 切流——跑的是旧默认或被配成 10s，违反自家庄不变式 server_silence > interval x max_missed（keepalive.rs 测试断言在守）。连带伤害：控制流一切 release_epoch_circuits 释放该代次全部电路，走该 relay 的 peer 连接同时死。
- 修法：服务端升级/改配 server_silence 不小于 45s，用服务端指标 keepalive_failures_total 验证（每周期 +1 即实锤）；客户端 keepalive interval 压到服务端窗口 1/3 以下（如 5s）。通用规则：凡周期性保活类参数，禁止与对端任一超时窗口同值或贴线。

## 2026-09-04 QUIC 拨号端点只绑 0.0.0.0：地址簿 IPv6 候选全数 invalid remote address（线上日志分析）
- 症状：每次重连地址簿逐个拨全失败，每地址 1-2ms 内报 invalid remote address: [fe80::...]（含全局 240e:: 段地址），直连跳 100% 全灭，流量全压 relay。
- 原因：QuicTransport 拨号端点绑 0.0.0.0:0（quic.rs new，纯 IPv4 socket），对任何 IPv6 目标族不匹配在本地即拒；地址簿候选又全是 IPv6（fe80:: 链路本地无 scope id 本就不可拨，还混进 fe80::1 路由器地址），filter_loopback 只滤 loopback 不滤 link-local。
- 修法：拨号端点双栈（绑 [::]:0 开 v4-mapped，或按目标地址族持双端点择路）；地址入簿/拨号前统一剥 link-local。通用规则：监听/拨号 socket 的协议族与地址簿地址族要用测试对齐，族不匹配应在入簿时拒，而不是拨号时逐个撞墙刷屏。

## 2026-09-04 reset_min_uptime 恰等于探测死亡窗口：0% 探测成功的会话被判健康（线上日志分析）
- 症状：三个 peer 会话寿命恒为 30.004-30.008s（3x10s 探测网格，首探即 early eof），每次重连都打 backoff reset: previous session healthy，退避永不升级，delay 恒 800ms、attempts 恒 1，30s 周期重连风暴无限循环。
- 原因：mark_connected 以 uptime 不小于 reset_min_uptime(30s) 判健康，而会话寿命恰被 max_probe_misses x probe_interval = 3x10s 钉死在 30s——「活得够久」与「探测成功」完全脱钩，参数互撞使健康判定形同虚设；且 EOF（对端关流/连接已死）与超时不分，白等满 3 次才断链。
- 修法：健康判定追加「本会话至少一次 probe 成功」；EOF 型探测失败立即断链不等满次数；新增超时参数时先核对与既有定时器网格（探测间隔/退避/保活）的倍数关系，避免语义相消。

## 2026-09-04 quinn 0.11 双栈端点两处 API/语义陷阱（T2 双栈化实测）
- 症状一：Endpoint::new 传 tokio::net::UdpSocket 报 E0308——quinn 0.11 的 new 直接收 std::net::UdpSocket，由 TokioRuntime 自行包装；先 from_std 再传反而类型不匹配。
- 症状二：V4 目标映射成 v4-mapped（::ffff:a.b.c.d）后 is_unspecified() 失真——0.0.0.0 变 ::ffff:0.0.0.0 不再未指定。quinn 的 connect_with 只做族校验（拒「V6 目标+非 v6 端点」），未指定地址/0 端口的确定性拒绝在 quinn-proto（endpoint 内层 connect），映射可令其被绕过，退化为吃满握手超时的悬挂。
- 修法：映射前对未指定地址契约性拒绝（Dial 文本变体）；双栈化令「族不匹配必拒」场景消亡后，既有 invalid-remote 契约测试改用 EndpointStopping（close 后拨号）确定性触发同一 source 链契约。改契约测试前先抄下「断言的契约本体」，再为新世界找等价触发。

## 2026-09-04 src-tauri 脱离根 workspace：门禁盲区里的破损潜伏一天才暴露（GUI 节点资料轮首次编译发现）
- 症状：feature 分支首次编译 apps/gui/src-tauri 即 E0423（`Arc::new(proto::EchoHandler)` 对带字段 struct 用单元构造）+ clippy -D warnings 红于 node_event.rs 冗余导入；根 `make check` 此前长期全绿。
- 原因：src-tauri 的 Cargo.toml 声明空 [workspace] 脱离根 workspace，根 fmt/clippy/test/panic-hygiene 四门禁与 gui-check（pnpm）都不覆盖它；6c4d882 改 p2p-cli 的 EchoHandler 形状（panic 卫生清零）时无人发现 src-tauri 消费点同步失效。
- 修法：任何改冻结 crate 公开形状的提交，先 `grep -rn "Sym" apps/gui/src-tauri` 查桥接层消费点（它是 p2p-cli 的 path 依赖方）；给该 crate 建议补一条 `cd apps/gui/src-tauri && cargo clippy -- -D warnings && cargo test` 进 gui.sh 或单独 gate，消除盲区。

## 2026-09-04 在只属于自己 scope 的 worktree 里跑全仓 cargo fmt：13 个历史文件被重排成噪声（GUI 节点资料轮）
- 症状：后台门禁跑完 `cargo fmt` 后 git status 冒出十几个自己从未碰过的 src-tauri 文件改动，且与并行会话的编辑面冲突风险陡增。
- 原因：src-tauri 历史提交本就未过 rustfmt（fmt 门禁只管根 workspace），在 src-tauri 目录里跑 `cargo fmt` 会把整个目录重排一遍——「格式化自己」变成了「格式化全目录」。
- 修法：scope 纪律的格式化只对自建文件手工保证风格（贴临文件写法），不动目录级 fmt；误重排的噪声文件用 `git checkout --` 还原、自己的改动按文件清单重新落补丁。连带教训：checkout 后必须立即读回验证生效，本例首次在 src-tauri workdir 下 checkout 未生效，改从仓库根用全路径重跑才成功。

## 2026-09-04 勘误：日志「串址」实为同机多实例共享接口地址（IP 同、端口异）
- 勘误对象：当日「QUIC 拨号端点只绑 0.0.0.0」分析中的「跨节点串址」论断。复核日志：DZvczj 与 EhUaawMPK 重叠的 3 个地址是 IP 相同、端口不同（/u58245 vs /u64802）——TransportAddr 含端口，二者并非同一地址；系同一物理机跑两个实例（lab 同机多节点部署形态），共享全部接口地址合法，且 macOS 多接口 + 隐私临时 v6 地址本就多达十余个。
- 教训：判定「串址」必须比对含端口的完整 TransportAddr；共享 IP+异端口优先怀疑同机多实例，再怀疑污染。本轮 false alarm 源于只比对 IP 就下结论。
- 复盘挖出的真缺陷：mdns decode_txt 只取地址集第一个 IP（iter().next()），多接口主机的其余合法候选全被丢弃；地址簿只增不汰，macOS 临时 v6 轮换留下死地址助长拨号风暴。修法见 fix/mdns-all-addresses：解码全量展开，去重/链路本地过滤统一由地址簿入簿卫生把关。

## 2026-09-04 P1 原语轮
- 集成测试用 NodeBuilder 默认 data_dir（./p2p-data）会把 key.seed 落进 crate 目录并被 git add 卷进提交——tests 一律 .data_dir(temp_dir)，.gitignore 兜底 p2p-data/；已误入的用 git rm --cached + amend 剔除（提交未扩散前强推自愈）。
- cargo fmt 必须是提交前最后一步：fmt 之后又改代码（哪怕一行）再提交，make check 的 fmt-check 必红；本轮 static_peers itest 两次实锤。收尾命令顺序固化为 fmt → test → fmt --check → commit。
- 广播事件在订阅前发送即丢：验证「启动时载入产生的登记/事件」不能靠 subscribe 后 try_recv，要么提前订阅要么走状态查询口（peer_registered/peer_addrs）——地址簿恢复类断言一律用状态口。

- T36 任务书验收命令路径缺陷（2026-09-03）：`cd apps/gui/src-tauri && ... && cd ../.. && make check` 中 src-tauri 距仓库根三层，`cd ../..` 落在 apps/（无 Makefile）导致 make 必然报 No rule to make target。修正应为 `cd ../../..`。执行会话按修正版跑通全量门禁，字面命令 exit 2。
- 主树 Makefile check 的 test 阶段跑 `cargo test --workspace`，p2p-chat 固定端口测试（31101+）与其他会话并行时存在已知 flake（协调挂账 1b07415），边界测试新增文件一律用随机端口（Node::builder 默认 0）。
## 2026-09-04 DSH：GLM-5.3-Flash 发图报「当前模型不支持图片」——配错键名，pi-ai 认 `input` 不认 `inputModalities`
- 症状：GUI（127.0.0.1:18181，bigmodel/GLM-5.3-Flash）发送图片即报「当前模型不支持图片」，但该模型明明支持视觉。
- 原因：`~/.dsh/settings.yaml` 的 `llm-pi-ai.providers.<route>.models[]` 条目写了 `inputModalities: [ text, image ]`——这是 llm-deepseek 插件的目录键名；llm-pi-ai 的模型 schema（config.ts `modelFields`）只读 `input`，未知键静默透传不被读取，模型落到 `defaultInput: ['text']`，session-controller 的图片准入门（`MODEL_DOES_NOT_SUPPORT_IMAGES`）按 text-only 拒收。
- 修法：pi-ai 各路由模型条目键名改 `input: [ text, image ]`；`llm-deepseek` 段的 `inputModalities` 是正确键名不要动。改完无需重启：settings-file 默认 chokidar 监听（watch: true），pi-ai 每请求重读 profiles，下次请求即生效。
- 验证手法：不改服务进程，用仓库真实解析代码直跑配置文件——`node --input-type=module` 导入 `packages/llm/llm-pi-ai/src/config.ts` 的 `resolveProfiles`（Node 24 原生 type-stripping），遍历 `provider.getModels()` 打印每模型 `input`，看到 `bigmodel / GLM-5.3-Flash -> input: [text, image]` 即闭环。注意 piProvider 没有 `.models` 属性，取模型要走 `getModels()`。

## 2026-09-04 gui 测试辅助文件含 JSX 存 .ts 必炸
- 症状：vitest 报 Transform failed Unexpected ">"，指向 fixtures 文件本身（如 chat-render-matrix-fixtures.ts:99），容易误判成测试文件语法错。
- 原因：fixtures 写了 JSX（render(<Toaster/><ChatView/>)），.ts 不走 react 插件的 JSX 转换。
- 修法：含 JSX 的测试辅助文件一律 .tsx 后缀；或 fixtures 保持纯数据构造（chat-boundaries-fixtures.ts 先例），把挂载/渲染装配留进 .tsx 测试文件。
## 2026-09-04 bash EXIT trap 引用函数 local 变量，set -u 下 unbound 污染退出码（R2 发布脚本轮）
- 【主树 node_modules 被 worktree install 污染】多轨并行期某会话在 worktree 内 pnpm install 后，主树 apps/gui/node_modules 的包符号链接被重写为指向该 worktree 私有 store；worktree 收尾删除后链接悬空，主树跑 vitest 报 MODULE_NOT_FOUND vitest/vitest.mjs（2026-09-04 实证，T48 收官时暴露）。症状识别：pnpm test 挂在 loader 而非断言、Node 尾栈+空 grep 结果。修复：rm -rf apps/gui/node_modules + 根目录 pnpm install 重建（--force 无效，pnpm 信任 .modules.yaml 状态）。预防：worktree install 后 readlink 主树包链接抽查；主树验收遇 MODULE_NOT_FOUND 先查悬空链接再怀疑代码。
- 症状：脚本全部步骤日志正常、末行 marker（R2-RELEASE-OK）也打印了，退出码却是 1，且报错出现在所有输出之后：`line N: tmp: unbound variable`。表面看「构建+冒烟全过」，机械验收（看退出码）却判 FAIL。
- 原因：`trap 'rm -rf "$tmp"' EXIT` 在脚本退出时才触发，此时定义 tmp 的函数早已 return，`local tmp` 出了作用域；set -u 下 trap 内引用即 unbound，trap 失败把退出码从 0 改写成 1。trap 体是退出时才求值的字符串，不是设置时的快照。
- 修法：trap 引用的变量在脚本顶层声明为全局（如 SMOKE_TMP=""），函数内只赋值；或设置 trap 时用双引号内插固化字面值 `trap "rm -rf '$tmp'" EXIT`。教训：带 marker 输出的脚本，验收必须看退出码而非 grep marker；自测时末尾追加 `echo EXIT=$?` 一次就抓到。


## 2026-09-04 IM-T48 CDP evalJs 解包错层静默 undefined
- 症状：CDP 驱动脚本"跑通"、截图正常产出，但所有 evaluate 返回 undefined，notes 全是 undefined，依赖返回值的分支判断全部失效；早期连报错都没有，走查空转三轮约 40 分钟。
- 原因：Runtime.evaluate 响应双层嵌套 {id, result:{result:{...}}}，send 解包了 m.result 而 evalJs 按 m 的形状取 r.result.result?.value，可选链把错误吞成 undefined。
- 修法：探针先行——脚本第一步 evaluate 'alive' 打印原始响应 JSON 核对层级，再定取值路径；业务断言对 undefined 立即 fail 而非继续。

## 2026-09-04 IM-T48 vitest 绿但 tsc build 红（noUnusedLocals）
- 症状：验收链在 pnpm build 断（TS6133 'get' is declared but its value is never read），vitest 全程绿。
- 原因：vitest 不做类型检查；重构后函数签名留下的未用参数只有 tsc 抓。
- 修法：每个 fix 提交前至少跑一次 pnpm build；未用参数直接从签名删除并同步全部调用点。

- vi.hoisted 里 vi.fn 泛型签名必须与 mockImplementation 实参一致：声明为零参（() => Promise<T[]>）后传带参实现（(peer: string) => ...）会在 tsc -b 阶段 TS2345（Target signature provides too few arguments），测试期绿、构建期红——mock 类型照抄真实方法签名（peer, beforeId?, limit?）。

## 2026-09-03 CL2 轮：cargo test 不更新 target/debug 二进制
- 症状：源码已修、跑 apps/cli/target/debug/p2pctl 行为依旧（control.rs call_slow 漏拆 {ok,data} 包装，客户端报 missing field）。
- 原因：cargo test 构建的是 test-harness 二进制（target/debug/deps/p2pctl-*），不产出 target/debug/p2pctl。
- 修法：验证可执行文件行为前显式 cargo build；或调试期统一 cargo run。E2E 脚本依赖预构建二进制时同理。

## 2026-09-03 CL2 轮：BSD sed/perl 正则处理含中文模式
- 症状：perl -0pi 带中文替换串直接 exit 255；sed -E 模式含 `1.2.3.4/u3400` 类地址报 "parentheses not balanced"。
- 原因：BSD 工具对多字节字符按字节处理破坏正则结构；正则里的裸 / 与默认分隔符冲突。
- 修法：含非 ASCII 的文件改动用编辑工具精确替换；sed 正则含 / 时换 | 分隔符。


## 2026-09-04 IM-T46A 轮纠正：仓库外 worktree 本会话全程可用
- 现场纠正下条（2026-09-03 CL3 轮）：本会话 worktree 建在 /Users/imeepos/ext512/wt-im-reply-backend（仓库外、与仓库根同级），read/edit/write/glob 全程正常，未被视图边界拦截。下条经验不是普适规律，是否受限取决于会话工作区配置；保险起见仍可优先 .worktrees/，但别因「仓库外必炸」的预判改变工作规划。

## 2026-09-03 CL3 轮：DSH 会话双视图——worktree 建在仓库外文件工具看不到
- 症状：bash 里 git worktree add ../p2p-xxx 成功（git worktree list 可见），但 read/glob/grep/write/edit 全部 not found；同窗口 bash 还间歇性 spawn ENOENT。
- 原因：DSH 文件工具视图以会话工作区根（仓库目录）为挂载边界，根外的同级目录不在视图内；bash 走独立通道与文件工具不同视图，且偶发 worker 故障。
- 修法：worktree 一律建进仓库内 .worktrees/（.gitignore 已收录、仓库有先例），bash 与文件工具即可同视图操作；已建在外的用 git worktree move 挪进来。bash ENOENT 是瞬时的，整程序级重试即可恢复。

## 2026-09-04 GC1 轮：管道后的 exit code 是 tail 的——构建假绿
- 症状：`cargo build 2>&1 | tail -40` 后台任务报 exit 0 且仅 13 秒「完成」，实为依赖 feature 解析失败（退出码是 tail 的，真错只在输出里）。
- 修法：判结果的命令统一 `set -o pipefail`，并在输出末尾落 `echo BUILD_EXIT=$?` 再判；验证「真编译过」看 Finished 行而非退出码。

## 2026-09-04 GC1 轮：objc2 生态 0.3.x 的 feature 与签名靠记忆猜→连环编译红
- 症状：objc2-app-kit/web-kit feature 名按旧印象写（NSBitmapImageFileType、WKWebView_takeSnapshotWithConfiguration_completionHandler）→ cargo resolve 阶段就拒；换类粒度 feature 后又炸 alloc 不可见、block 参数类型不匹配。
- 原因：0.3.x generated crates 的 [features] 是类粒度（NSImage/NSBitmapImageRep/WKSnapshotConfiguration…），方法级门控由类 feature 组合依赖 feature（如 takeSnapshot 是 all(WKSnapshotConfiguration, block2)）；completion handler 用裸指针 `*mut NSImage`（不是 NonNull），`alloc()` 需 `use objc2::AnyThread`，handler 参数是 `&DynBlock` 非 Option。
- 修法：写码前先读本机 `~/.cargo/registry/src/<registry>/objc2-*-0.3.2/` 的 Cargo.toml [features] 与 generated/<Class>.rs 真实签名，一次写对。

## 2026-09-04 IM-T45 轮：多会话并行 cargo test 时端口型 itest 互踩假红
- 症状：make check 挂在 p2p-itest --test peer_lifecycle（exit 101），该 crate 与本单改动（apps/gui）零交集。
- 原因：多个并行会话同时在各自树上跑 make check，端口/时序型集成测试互踩；同树单测复跑 3/3 全绿（0.73s）即证 flake。
- 修法：先单测复跑定性——红≠自己 diff 的锅；定性后整链重跑验收，全绿再合并。

## 2026-09-04 devloop_accept 无视 root 参数，命令跑在工具默认 projectRoot
症状：devloop_accept({root: "/Users/imeepos/ext512/p2p"}) 的验收命令实际在
wt-t36-boundary 执行（vitest 栈路径实证），且 exit null 被记为 failCount 1。
原因：命令执行用工具自身配置的默认 projectRoot，root 参数只影响账本读写。
修法：在目标树手动跑 acceptanceCommand 拿权威 exit code，再用
devloop_ledger(action=save) 修正任务记录（note 写明误跑根因）；
并行会话 worktree 里跑别人 WIP 的测试有污染面，重跑前先确认。
## 2026-09-04 GC2：WKWebView takeSnapshot 回调永不到达——with_webview 闭包内阻塞等待主队列回调互锁
- 症状：控制通道 /screenshot 恒返 CAPTURE_FAILED「快照完成回调 4s 未到」；`sample` 佐证主线程全程在事件循环、recv 栈全在非主线程；权限预检通过后必现，截图录屏（共用 FrameSource）全灭。
- 原因：RealFrameSource::capture 在 with_webview 闭包内同步 recv_timeout(4s) 等 takeSnapshot 完成回调——回调走主队列，闭包占着主线程（或非主线程调 WKWebView 致回调不派发），互锁必超时。集成测试全用合成帧源，真实快照路径自声明「由 GUI 运行态验证」却从未验证——「文档说验证过」不等于验证过。
- 修法：闭包内只发起快照立即返回，完成回调（WebKit copy block，主队列异步执行）持有 channel 发送端回传结果；等待统一收敛到服务线程既有 5s 外层 recv。修后真机 60KB PNG / 51KB GIF 正常产出。


## 2026-09-04 相对 core.hooksPath 在 worktree add 时被静默跳过
症状：git config core.hooksPath githooks（相对路径）后，从主树发起 git worktree add，post-checkout 钩子完全不执行（无输出无报错），新 worktree 缺 .env；换到有 githooks/ 检出的目录发起就正常。
原因：worktree add 触发钩子时，相对 hooksPath 相对"发起命令时的 cwd"解析，不是相对新 worktree 也不是相对 GIT_DIR；目标目录没有钩子文件 git 就静默跳过，无任何警告。
修法：引导一律写绝对路径 git config core.hooksPath "$(pwd)/githooks"（在仓库根执行）；诊断钩子未触发先用 echo 哨兵钩子验证是否真的被执行、从哪个 cwd 解析。

## 2026-09-04 N2：quoted heredoc 里手工转义反引号 → 字面反斜杠落盘
症状：cat >> file 加 quoted heredoc 追加的 markdown 里出现反斜杠加反引号的转义残骸，文档渲染破相。
原因：quoted heredoc 本就不做任何展开，内容里的反引号原样安全；在 JS 侧先
replace 转义再过 heredoc，等于双层转义，反斜杠被当正文写进文件。
修法：quoted heredoc 内容零转义直写；已污染用 python3 replace(chr(92)+chr(96),
chr(96)) 修复并断言计数，别用 sed 硬拼正则。

- 2026-09-04 N1：run_code 里 tools.bash 报 "binding arguments must be lossless JSON"。
  症状：同一段 helper 代码时好时坏，报错点指回 bash 绑定调用。
  原因：description 等参数动态计算（如 command.slice(0,60)）或单次调用塞多条长命令，
  序列化失败；与命令内容本身无关。
  修法：description 一律静态字符串；复杂批次拆成单条调用，每条一次 run_code。
- 2026-09-04 N1：bash 里 grep 变量模式未加 -e，模式以 -- 开头（如 --json）被当
  grep 自己的选项，报 "unrecognized option" 门禁红得不知所云。
  修法：一切变量模式写 grep -Fxq -e "$var"；写门禁脚本时逐处检查。
- 2026-09-04 N1：负向注入测试（人为改坏文件验证门禁变红）后想用 git checkout 恢复，
  但文件还未 commit（untracked），checkout 报 pathspec 错误无法恢复；后续负向测试
  在已污染文件上叠加，现象互相干扰。
  修法：负向测试前先把干净版本 commit（有 ref 才能恢复）；或准备好原始内容随时重写。
- 2026-09-04 N1：tools.write 报 "file changed since it was read"——chmod 等元数据
  操作也算改变。修法：重读一次该文件再写。

## 2026-09-04 IM-T50：并行轮共享 target 的两类验收假红（CARGO_BIN_EXE NotFound / vitest 负载超时雪崩）
- 症状一：make check 的 cargo test 在 repair-bridge boundary 报 Command::new(env!("CARGO_BIN_EXE_repair-bridge")) NotFound，而该二进制实际存在且 40 分钟前构建；隔离复跑 cargo test -p repair-bridge 9/9 绿。
- 症状二：同轮主树 vitest 3 用例撞 5s/30s 超时上限 + 1 例「找到多个取消按钮」；同码 worktree 侧 256/256 全绿。
- 原因：load 34-45 下多会话并发 cargo/vitest——集成测试起子进程时 bin 目标正被并行构建短暂移换；vitest 用例 CPU 配额被挤压，RTL waitFor 类断言在慢时钟下偶发失真。
- 修法：全量验收红先三步定性——隔离复跑最小面（cargo test -p 单包 / 单文件 vitest）、查产物 mtime、同码他树全绿对照——确认环境竞态后错峰重跑，不对环境红改代码；与 IM-T45 端口互踩条同族，本条补二进制竞态与负载超时两个变种。

## 2026-09-04 cargo 验收链挂死持锁——clippy/check 全家排队 25 分钟根因
- 症状：任意 cargo 命令停在 "Blocking waiting for file lock on build directory" 半小时以上无输出。
- 根因：另一 cargo 进程挂死未死透仍持 target/.cargo-lock（flock 随进程存活，进程不死锁不放）；cargo 锁等待无超时参数，只能干等。现场：凌晨 2:19 的 t49 验收链 cargo test --workspace 卡在某测试 Running 后，持锁 15 小时全程 0.13s CPU，后续所有 cargo 全部排队；另一条 CARGO_TARGET_DIR 隔离链也挂死在 Compiling 中途（rustc 子进程全消失、无网络句柄），挂死不限于测试阶段。
- 定性：ps 看 TIME(CPU 秒)/ELAPSED 比 + pgrep -lP <pid> 看子进程，CPU≈0 且无子进程超 10 分钟即挂死（编译中必有 rustc 子进程且 CPU 上涨）。
- 修法：从最老祖先 kill 整链（bash 包装 + cargo）；锁随进程死亡立即释放，无需删 .cargo-lock 文件（其 mtime 与锁无关）。
- 预防：后台验收链整链包 timeout + CARGO_TARGET_DIR 指独立目录 + 测试用随机端口（本文件 310 行固定端口 flake 同族）。

## 2026-09-04 IM-T49：根 workspace clippy 是假阴性——不覆盖 apps/gui/src-tauri 独立子 workspace
- 症状：仓库根 `cargo clippy --all-targets -- -D warnings` Finished 零告警，随后在 apps/gui/src-tauri 内跑同命令却报 error（clippy 1.98 needless_borrow，control/handlers.rs）。
- 原因：src-tauri 是独立 workspace（自带 Cargo.lock），不是根 workspace 成员；根级 clippy 根本不编译 p2p-console。「根目录 clippy 绿」不能作为全仓门禁绿的证据。
- 修法：验收链必须在 src-tauri 目录内单独跑一次 clippy（账本命令本就如此编排）；给 make check 补独立 workspace lint 覆盖已立 OPS-P1 治理卡。

## 2026-09-04 IM-T49：线上契约加字段无 serde(default)，旧帧跨版本必炸
- 症状：采纳 T36 旧测试时，含「信封缺 replyTo 字段」的原始线上帧在当前 main 全部解析失败断流。
- 原因：T46A 给 WireEnvelope 加 reply_to 未标 serde(default)——Option 字段没有 default 时反序列化仍要求键存在；线上格式无版本协商，旧版本发出的帧直接被判非法。
- 影响与修法：契约演进规则=线上结构体加可选字段必须带 serde(default)（本地落盘结构已有「缺字段读回」容忍惯例可循）；当日以「缺 replyTo 断流」对抗用例锁定现行为，是否补 default 交协调裁决。

## 2026-09-04 IM-T49：git 工具链断点汇总（当日三踩）
- 见 techniques 同日条：amend 错位打 HEAD、fixup 目标被 amend 后 autosquash 静默落空、`| tail` 吞退出码——三类都让「提交历史看似正确实则内容错位」且绿标误导，重整后必须 `git show --stat` 逐提交核对文件归属再加验证。

## 2026-09-04 pkill/pgrep -f 自匹配连环自杀（一晚三踩，跨机 ssh 演练）
症状：ssh 远程「pkill 掉旧进程再跑新命令」的复合命令，exit 255 且零输出，或输出在 kill 后戛然而止；误判为网络/服务问题排查了一圈。
原因：pkill -f 的模式串以各种形态出现在自身命令行里——①模式原文直接在命令里；②模式的前缀出现在同一命令行其他参数（如 rm -rf 的路径、业务命令的 --data-dir）恰好能被正则命中；kill $(pgrep -f ...) 同理。远程 shell 的 cmdline 含整条命令文本，匹配即自杀。
修法：①模式中间字符用字符类打断字面匹配（chat serv[e]）；②同一命令行内不得出现任何能被该正则命中的纯文本（含 rm 路径、业务参数）；③最稳形态：先单独 pgrep 存 PID 变量核对，再单独 kill；进程管理命令与业务命令拆成两次 ssh。

## 2026-09-04 GC3：dispatch_task 600s 墙钟超时后的接管甄别
- 症状：dispatch_task 大型多文件任务 600s 墙钟上限被砍，报错只有 timeout 无产物清单；如果直接重派或自己重写，会和子代理的半成品双写。
- 原因：DSH dispatch 有硬墙钟上限；任务内含冷 cargo 全量构建时必然超（见 techniques 同日条：沙箱 fs 开销下 rustc 0% CPU 伪挂起）。
- 修法（AGENTS.md 已固化「接管前先查产物再动手」）：git status + git log + 新增文件逐个 read 甄别子代理产物质量；合格则保留续作（本例前端 6 文件全部保留），缺什么补什么；确认超时前子代理未碰的区域（本例 Rust 侧零产物）才自己动手。

## 2026-09-04 GC3：git worktree add 中断留半成品目录 + 悬空分支
- 症状：worktree add 输出停在 "Updating files: 68%" 后命令返回；git worktree list 看不到该 worktree，但分支已建成、目录里只有部分文件（.git 文件缺失）。
- 原因：bash 工具层中断打断了 checkout 中段；分支 ref 已写、worktree 注册未完成。
- 修法：rm -rf 残留目录 → git branch -D 悬空分支 → 重新 worktree add（给足 timeoutMs）；重建后 ls 抽查关键子目录（如 apps/gui/src/views 与 src/stores 同时存在才算完整）。
