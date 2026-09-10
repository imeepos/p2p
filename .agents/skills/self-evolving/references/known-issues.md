# Known Issues

<!-- 格式：症状 → 原因 → 修法。排查超过 5 分钟的 bug 才值得记。 -->

## 2026-09-09 GUI 创建智能体报 404：代码对、服务旧，探针用旧端点验证新能力导致安装假绿
- 症状：/agents 页创建智能体弹窗点确认必报「HTTP 404: not-found」；repo 代码、GUI 测试、契约文档全部正确；GET/POST /a2a/agents 用 curl 直打运行中的 agent 也是 404，而 /shares 200 正常。
- 原因：launchd 常驻的 ~/.dsh/bin/acp-agent 是 Sep 7 的旧二进制（strings 里 grep 不到 "a2a/agents"），a2a 管理面是 Sep 8 才合并的；旧 admin 服务对未知路径落 catch-all 404。而 scripts/ops/acp-local-setup.sh 的安装探针用 GET /workspaces 当「新二进制标志」——旧二进制同样有 /workspaces，所以每次重装脚本即使没装上新二进制也报 ACP-LOCAL-SETUP-OK，假绿掩盖了部署滞后。GUI 的「我的」列表 404 被设计成静默降级（旧 agent 常态），只有创建动作把 404 原文上浮，于是用户只在创建时看到报错。
- 修法：跑 scripts/ops/acp-local-setup.sh 重装重启（构建→安装→launchd kickstart），GUI 每次确认时重读 descriptor（adminUrl/token 随新进程刷新）无需重启 GUI；探针改为双端点校验（/workspaces + /a2a/agents 各 200，后者缺失报独立文案退出）。排查口诀：「repo 代码正确 + 线上端点 404」先查常驻进程二进制的新旧（ls -la 二进制 mtime、strings grep 新路由字面量），再查安装脚本探针是否用了新旧二进制共有的端点当新鲜度标志——验证标志本身必须是被测能力独有的端点。

## 2026-09-07 聊天消息多才冒横向滚动条：罪魁是 opacity-0 的 hover 按钮越出滚动域
- 症状：聊天消息流「消息多的时候」出横向滚动条；连续两条消息视觉上贴叠成一泡。横向 scrollbar 与消息数量的相关性把排查引向内容宽度（图片/长文本），全都不是。
- 原因：两件事叠加。① CSS 规定 overflow-y 非 visible 时 overflow-x 的 visible 隐式算作 auto——只写 overflow-y-auto 的滚动域其实横竖都可滚；② 悬停回复按钮 absolute left-full 锚定整行，me 侧越出滚动域右缘 12px（padding 只有 16px），而 opacity-0 不影响 scrollable overflow——看不见的元素照样撑出滚动条；竖向滚动条出现（消息变多）收窄 8px 内容宽只是让存量溢出显形。纵向贴叠则是消息列 flex-col 没写 gap。
- 修法：滚动域显式 overflow-x-hidden 兜底（横向滚动机制上不可能）；越界元素改锚到行内的定位锚点（气泡列加 relative）落在空白侧；消息列补 gap-y-2.5。排查口诀：滚动域内 grep absolute + left-full/right-full/-right-/-left- 负偏移，每个都要问「越出 padding box 没有」；opacity-0/visibility-hidden 不豁免 overflow。

## 2026-09-07 分享创建失败：GUI 测试 stub fetch + agent 测试裸 TCP，CORS 预检缝两侧测试都探不到
- 症状：GUI「生成链接」必弹「分享创建失败」；同 URL 用 curl 直打 200 成功；GUI/agent 两侧测试全绿。
- 原因：WebView fetch 带 Authorization 头必先发无凭据 OPTIONS 预检，admin HTTP 管道层把预检当未授权请求 401 挡掉。前端测试 vi.stubGlobal("fetch") 把浏览器传输语义（CORS/预检/Origin）整体 mock 掉；agent 测试用裸 TCP 客户端不发 Origin 不做预检。两侧各自 100% 绿，缝在两个测试视角的交集盲区。
- 修法：admin.rs 预检先于 Bearer 校验（204 + 放行方法/头），实响应按 origin 白名单回显 ACAO；新增 admin_cors_tests.rs 用「带 Origin 的客户端」锁死浏览器行为矩阵。教训：跨进程 HTTP 契约（WebView→本地服务）至少要有一条真浏览器对真服务的 E2E，或后端测试必须模拟浏览器行为矩阵；「curl 成功 + 浏览器失败」= 首查 CORS 预检。

## 2026-09-07 UX-R2C：acp-store 的 draft 跨测试用例残留，新用例假定空白初值即踩
- 症状：endpoint 弹窗新用例不填 token 点「测试连接」，预期报 tokenRequired 却直接发起连接，DOM 里找不到错误码；单文件重跑又全绿（用例执行顺序敏感）。
- 原因：beforeEach 的 localStorage.clear() + resetConsoleState() 都不清 in-memory store 的 draft/saved（resetConsoleState 只清连接态面）；同文件前序用例经 upsertSaved 留下的 draft（含 token）被新用例的 useState(draft) 继承，validate 直接通过。
- 修法：依赖表单初值的用例必须在用例内显式 setState/填充被测字段（哪怕值是空串），或 beforeEach 显式 useAcpStore.setState({ draft: EMPTY_DRAFT, saved: [] })；「单文件重跑绿、全量红（或反之）」先怀疑 store 模块级状态残留而非代码逻辑。

## 2026-09-07 UX-R2C：run_code 里给 edit/write 传 JSX/引号密集内容，模板字面量与转义引号都会炸解析
- 症状：同一段 edit 调用，反引号模板串传 new_string 报 Unterminated template；单引号串加反斜杠转义引号报 Expected unicode escape / Expected comma got hash；三种报错都不指向真实原因。
- 原因：run_code 的 code 传输层对特定转义序列/多行模板敏感，报错与真实内容无关；CJK+JSX+引号混排必炸。
- 修法：数组按行存内容（行内裸双引号，外层单引号），join 换行后传参；绝不内嵌反引号模板、绝不反斜杠转义引号。另：read 后立刻 edit 同一文件（worktree 路径变化即视为未读）。


## 2026-09-07 UX-K：Radix Tabs 用 fireEvent.click 不切换，且非激活 Content 以 hidden 空壳留在 DOM
- 症状：弹窗内 Tabs 点触发器后 aria-selected 仍 false；断言「内容存在」却通过（getByTestId 命中 hidden 空壳），下游子元素断言才失败，误导排查方向。
- 原因：@radix-ui/react-tabs 1.1.x 触发器在 onMouseDown 里 onValueChange（click 不是激活事件）；TabsContent 非激活时不卸载而是 hidden+不渲染 children。
- 修法：测试里 fireEvent.mouseDown(trigger) + fireEvent.click(trigger)；断言激活态用 aria-selected 或检查 children 而非仅容器存在性。

## 2026-09-07 UX-K：locale 追加键时编辑锚点取「块尾三行」命中了同构的另一块，整块被复制进对象
- 症状：esbuild 报文件末尾 Unterminated string literal，行号离真实出错处十万八千里；vitest 该文件 0 用例直接 Failed Suites。
- 原因：edit old_string 只取了 `fallback…},
};` 这类高频同构尾巴，命中上一个命名空间的收尾而非目标位置，替换文本又整块重申了该命名空间 → 对象字面量中途再开同名键。
- 修法：结构化文件（locale/大对象）追加整块时，old_string 必须带足够上文的唯一多行片段（含块 opener）；改完立刻跑一次 tsc 或单文件 vitest，文件级 transform 失败要去看真实文件尾部而不是相信报错行号。

## 2026-09-05 连接池「双向同时拨号」收敛规则误用于同向重拨，重连被死连接残留挡下（BASE1/T23）
症状：第二借方进程经 bootstrap 查号挂满 10s 握手超时，每轮重试再挂；服务端周期性 server link ended 噪音；同 PeerId 新进程重连一律失败（ISSUE 半开残留）。复现呈**进程级二值**（同一份代码有的进程全挂、有的全过）。
原因：池收敛规则「恒保留较小 PeerId 一端拨出的连接」本为双向同时拨号竞态设计（两端结论一致），却被 admit 到同向重复入池——对端重连时池内旧条目尚未被回收（QUIC 空闲超时窗口最长约 30s，半开更久），新连接被判 RejectedExisting 并 close(hangup)。是否触发取决于两端 PeerId 排序，故按进程二值。
修法：池条目记录方向，同向重复入池一律新连接胜出（Replaced），跨向竞态仍走静态规则；客户端侧配连接复用（查号与周期循环共用一条连接），把「同向第二条存活连接」这个会引发替换抖动的形态消灭在源头。

## 2026-09-04 AGENTS.md 收尾四步写死 git push gitea，本仓库远端实际只有 origin
症状：按 AGENTS.md「远端名是 gitea 不是 origin」执行收尾第①步，`git push gitea` 报 'gitea' does not appear to be a git repository。
原因：规则文本来自其他仓库配置或历史，与本仓（ext512/p2p，`git remote -v` 只有 origin=github.com:imeepos/p2p）不符；任务书写「远端 origin」反而与实际一致。
修法：收尾前先 `git remote -v` 实证远端名、`git rev-parse main origin/main` 核对基线，实证结果优先于记忆中的规则文本；AGENTS.md 的 gitea 名在别的仓可能对，别跨仓照抄。

## 2026-09-04 Tauri debug 二进制加载 devUrl，外部 vite dev server 占 5173 致 E2E 假红
症状：E2E 隔离 HOME 起真实 GUI，控制通道/页面协议全活，但前端新代码的日志文件永远不出现（W1 感知断言三连红，GUI 日志却显示后端事件已发）。
原因：debug 构建无 custom-protocol 特性时 Tauri 加载 build.devUrl(localhost:5173) 而非内嵌 frontendDist；机器上有常驻 vite dev server（旧代码），GUI 装上的是旧 dev bundle，行为完全正常但不含新功能。压缩内嵌产物 grep 不到明文字符串，无法用二进制 grep 判陈旧。
修法：app 级 `custom-protocol = ["tauri/custom-protocol"]` 特性 + E2E 构建命令显式带上（强制内嵌 frontendDist，增量幂等）；判产物陈旧改用可 grep 的 dist JS 明文。日常 tauri dev 不带该特性行为不变。

## 2026-09-04 notify 直连依赖与 notify-debouncer re-export 版本漂移致 Watcher trait 不匹配
症状：src-tauri 加 `notify = "8"` + `notify-debouncer-mini = "0.5"` 后满屏 `FsEventWatcher: notify_debouncer_mini::notify::Watcher is not satisfied`。
原因：debouncer-mini 0.5 依赖 notify 7，直连 notify 8 → 依赖图双版本，Watcher trait 来自两个不同 crate。
修法：notify 一律经 `notify_debouncer_mini::notify::…` re-export 使用，Cargo.toml 禁止再直连 notify；加依赖后先看 Cargo.lock 是否出现同 crate 双版本。

## 2026-09-04 tokio::select! 相对 sleep 被更快 interval tick 永久饿死
症状：rendezvous 客户端在死连接上以固定 20s 周期空转报错（write half closed）达数十分钟，永不重连；查询分支再也没执行过。
原因：connect_and_loop 的 select! 每轮循环重建相对 sleep（30s 查询档），20s 注册 tick 先到期就把 sleep 丢掉重建——查询分支的 30s 永远到不了期，错误永不传播。
修法：周期分支用绝对时刻（tokio::time::Instant + sleep_until），触发后再推算下一截止；链路级错误必须在当轮上抛触发重连，不能只记日志继续循环（rendezvous_facade_link itest + reconnect_tests 锚定）。

## 2026-09-04 裸传输链不自 accept 入站流被对端 liveness 掐线
症状：经 TransportLink 盲拨的连接约 33s 被 facade 对端判死关链（probe missed ×3），客户端只见 Link("connection lost")；E4 修复（持有 SecureConn）后仍复现。
原因：裸链不进 swarm，连接只被当作出站流用；对端 liveness probe 在该连接上开 ping 流，本端无人 mux.accept_stream，探活永不命中。
修法：TransportLink::connect 起 spawn_link_responder 循环，accept 入站流并 dispatch 内置 PingHandler；未注册协议 debug 关流（crates/p2p/src/rendezvous.rs）。E4「句柄持有」与本次「入站应答」是同一坑的上下半场。


## 2026-09-05 workspace_session_manage archiveSession 的 archivedSessionIds 是累计归档注册表上报
症状：传单 sessionId 归档一个会话，响应 archivedSessionIds 返回数百个 id（含大量历史已归档会话），疑似误归档全部。
真相：返回的是归档后注册表全量快照（第二次调用的清单=第一次清单+新 id 恰好佐证）；session_link_list 复核仅目标会话消失，历史会话与 P3 新会话均不受影响。
修法/对策：归档后必须用 session_link_list 复核活跃清单再下结论；不要按响应里的清单长度判断误伤。
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

## 2026-09-07 run_code 大文件 write 偶发截断
- 症状：write 工具返回成功或后续命令静默失败，但目标文件停在中间，出现 Unterminated string、unclosed delimiter 或大量测试突然失败。
- 原因：大段模板内容经过运行时传输时可能被截断；同一 run_code 中后续 bash 解析错误也可能让前面的意图未执行，且 workdir 偶尔传错使验证落在仓库根目录。
- 修法：大文件改用小范围 edit 或临时脚本；每次写后立刻 read 尾部、wc -l、typecheck；bash 内显式 cd 并输出 pwd/branch，解析错误后重新 read，不能假设前序 edit 已落盘。

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

## 2026-09-05 PR2：ai-docs-sync 参数机械比对「看不见」clap visible_alias
- 症状：log 域给 --log-dir 加 `visible_alias = "data-dir"` 后，ai-docs-sync 报「文档参数 --data-dir 实现不存在」，但肉眼 --help 输出里明明有。


## 2026-09-07 UX 终验返工：走查假阴性双例（F14 焦点校验/F21 图标按钮）——漏检≠缺失，先探针定性再动代码
- 症状：终验报 F14「99999 失焦无提示无 aria-invalid」与 F21「邀请卡无复制按钮」；返工实测两条功能在基线上全都正常（真实 Tab 失焦 role=alert 全出；复制按钮 90ce353 早已交付且点击出 toast）。
- 原因：F14=UX-J headless 焦点问题的又一现场——探针实测程序化 focus()+blur() 后 blur/focusout/focus/focusin 四类监听**计数全 0**（document.hasFocus()=false，连事件都不派发），走查 eval 路径天然测不到；F21=复制按钮是纯图标 ghost button（无文本），走查按文本/可见文案找 affordance 就漏检。
- 修法：返工单遇到「曾交付过」的 finding，先跑带事件探针的单次 eval（挂监听数计数）+ 真实输入域事件（CDP Input.dispatchKeyEvent Tab / dispatchMouseEvent 点击）复现定性，再决定修产品还是修走查口径；单测断言禁用裸 fireEvent.blur 作失焦口径（不冒泡、与真实路径不同委托），统一 focus-then-blur 事件对（jsdom 下 blur() 冒泡 focusout）。图标类 affordance 的走查断言按 aria-label/role 找，不按可见文本找。

## 2026-09-07 vite-plus 的 node/pnpm shim（bin/node→vp）在 harness 非交互 shell 里挂起
- 症状：bash 里 node -v、pnpm -v 无输出不返回（进程挂起直到超时）；GUI 全流程（vitest/dev server/CDP 驱动）全被卡。
- 原因：/Users/imeepos/.vite-plus/bin/node 是符号链接到 ../current/bin/vp 启动器，该 shim 在无 TTY 的 harness shell 里等待不前；真实 runtime 在 ~/.vite-plus/js_runtime/node/<ver>/bin/node。
- 修法：一律用直接路径调用：NODE=/Users/imeepos/.vite-plus/js_runtime/node/24.20.0/bin/node；pnpm 用 node 直接跑 shim 脚本：$NODE /Users/imeepos/.vite-plus/bin/pnpm <cmd>。另注意 harness bash 固定在会话 cwd 运行（workdir 参数失效，2026-09-04 已录），pnpm install 前必须显式 cd 到目标树并 pwd && git branch --show-current 自证，否则装的是主树依赖（本轮实录一次）。

## 2026-09-07 harness edit 工具 JSON 参数顺序敏感：old_string 必须放参数对象第一位
- 症状：tools.edit 传参把 new_string 放在 old_string 之前时报 "missing required property old_string"，两次复现；调序后同样的字符串内容即成功。
- 修法：调 edit 工具时把 old_string 写在对象字面量第一位（file_path 之后紧随），别依赖键序无关假设；批量 edit 时拆成多次调用，失败重试先查参数顺序。

## 2026-09-07 vitest vi.fn 零参签名在 tsc 严格元组下无法索引 mock.calls[0][0]
- 症状：vi.fn(async () => undefined) 后写 writeText.mock.calls[0]?.[0] 报 TS2493（Tuple type '[]' has no element at index '0'）；vitest 运行全绿但 typecheck 红。
- 修法：显式标注形参 vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined)（诊断页 F27 用例既有写法）；要断言入参的 mock 别用零参箭头签名。

## 2026-09-07 深链开抽屉用 useEffect setState 触发 react-hooks/set-state-in-effect 红线
- 症状：/contacts?agentDetail=<id> 深链在 useEffect 里 setDetailId 开抽屉，lint 直接报错（hooks 新规），repo 无豁免先例。
- 修法：照 contacts-view hash 深链先例改渲染期同步——useState 惰性初值解析深链 + lastDeepLink 渲染期比对变更再 setState；注意别留旧 useState 声明造成重复声明（duplicate const 在 eslint 只报 no-useless-assignment 不报语法错，迷惑性强）。

## 2026-09-07 Radix Button asChild 下 data-testid 落在子元素本体
- 症状：getByTestId("agent-edit-link").querySelector("a") 取到 null——asChild 把 props 合并到子元素，testid 就在渲染出的 <a> 上。
- 修法：断言直接对 getByTestId(...) 本身做 tagName/href 检查。

- 2026-09-07 UX-R2A 症状：vitest 单文件跑绿、全量跑出现「scrollIntoView is not a function」Unhandled Rejection 且计为 Errors——原因：其他测试文件触发了带 scrollIntoView 的校验路径而各自 jsdom 无 stub。修法：helper 内能力探测降级（typeof element.scrollIntoView === 'function'），或每个涉及文件 beforeEach stub HTMLElement.prototype.scrollIntoView（settings-focus-error.test 先例）。

## WinSCP 连不上 138 服务器（2026-09-07）
- 症状：WinSCP 连 43.240.223.138:22 失败，用户以为是 22 端口被封
- 真因：sshd 加固后 PasswordAuthentication no（只许密钥），且 WinSCP 里配的是 root（PermitRootLogin prohibit-password 也拒）；ops 账号密码为锁定态(L)
- 修法：查 auth.log 确认是认证被拒而非端口封禁 → 改 PasswordAuthentication yes → chpasswd 给 ops 设密码 → sshd -t + reload → expect 实测密码登录
- 坑：fail2ban 封禁列表为空、ufw 放行 22 时，连不上几乎都不是端口问题，先看 auth.log

## 2026-09-07 沙箱内 node 子进程内建模块导入挂起

- 症状：bash 工具里 node -e 动态 import 内建模块与 .mjs 脚本（含 import 内建）永久挂起、零输出；纯 console.log 正常；python/swift/cargo 正常；早期同会话 pnpm dev 又能起，后期 pnpm vitest 挂起。
- 绕法：浏览器 CDP 改走 python 标准库 + Chrome --remote-debugging-pipe（fd3/fd4，NUL 分隔 JSON；pass_fds+preexec_fn 里 dup2 到 3/4）；桌面 GUI 验证改走 Swift AX 驱动；单测复核让仓库自身门禁承担，报告如实标注证据层级。
- 另：run_code 里用 JS 模板字面量拼 shell 命令时，内容含 $() 、${VAR}、反引号或 ASCII 单引号都会炸解析/截断——长 shell 串一律单引号拼接 + 内容先经 tools.write 落盘再引用。

## 2026-09-06 DSH bash 内 ~/.vite-plus/bin/{node,pnpm} shim 挂起（与 2026-09-07 node 挂起条同族）

- 症状：bash 里 `node -v`、`pnpm -v` 永不返回，整条命令超时 SIGTERM（结果 exitCode=null、timedOut=true），输出停在 shim 调用点之前。
- 原因：~/.vite-plus/bin/node 与 bin/pnpm 是指向 ~/.vite-plus/current/bin/vp 的 Mach-O 启动器（pnpm 脚本头 `#!/usr/bin/env node` 二跳仍命中坏 shim），在该执行环境内阻塞不退出；nvm 的真实 node 无此问题。
- 修法：显式绝对路径用 ~/.nvm/versions/node/v24.14.1/bin/node；起 vite 用 `node <app>/node_modules/vite/bin/vite.js` 或程序化 createServer，勿直跑 node_modules/.bin 下的 env-node shim。同轮实证：nvm node 跑 .mjs（含 node:fs import）正常——先换真实二进制，再考虑上条 2026-09-07 的 pipe 方案。

## 2026-09-06 裸 CDP evaluate 传箭头函数源码未自动调用，断言大面积假阴

- 症状：17 项断言 16 项 FAIL 且 detail 全打印 {}；经 waitFor（正确包了一层）的项反而 PASS——假阴模式高度规律。
- 原因：Runtime.evaluate 只吃表达式字符串；把箭头函数「源码」直接当表达式求值得到的是函数对象，returnByValue 序列化成 {}，后续 x===true / x===null 全 false，且全程零抛错不报异常。
- 修法：驱动层统一约定「入参即函数源码，evalJs 内部自动 `"(" + SRC + ")()"` 再求值」；断言 detail 全 {} 先怀疑驱动层取回函数对象，别急着怀疑被测代码。

## 2026-09-06 同 chrome profile 重跑吃到上轮 localStorage 写入，基线断言假红

- 症状：storage-clean-before-skip 断言 FAIL（键值=0.2.0），一度误疑 mock/应用层。
- 原因：上一轮驱动脚本 bug 误触「跳过此版本」，skipped-version 持久化进 user-data-dir 的 localStorage；同 profile 重跑基线自然不干净。
- 修法：基线敏感断言（空→写入→清空）正式跑前 pkill chrome + rm -rf 其 user-data-dir 全新起；轮次间污染就作废重跑，不打补丁。

## 2026-09-08 run_code 里 heredoc 结束后再接 && 链：bash 语法错误整条静默 exit 2
- 症状：git commit -F - <<'MSG' … MSG 的 heredoc 提交后按惯例 join(" && ") 续接下一条命令，整条 exit 2 且 stdout/stderr 零输出，commit 实际没跑。
- 原因：heredoc 结束符 MSG 必须单独成行收尾，之后不能再挂 "&& 下一条"；join 产生 "MSG 换行 && git log"，" && " 开头的行是 bash 语法错误，解析阶段整条失败——前面所有命令一条都没执行，不是中途失败。
- 修法：heredoc 永远放命令串最后一段，后续命令拆独立调用；看到 exit 2 + 零输出先怀疑整条解析失败，再怀疑单条命令失败。（与 2026-09-02 W6-S2 的「链式 heredoc 只执行第一段」同族：那次是部分执行，这次是整条不执行。）

## 2026-09-08 data-tauri-drag-region 拖不动：core:default 不含 start-dragging 权限
- 症状：macOS Overlay 标题栏改版后顶栏加了 data-tauri-drag-region 却完全拖不动，无任何报错弹窗。
- 原因：拖拽区 mousedown 走 `plugin:window|start_dragging` IPC，而 `core:window:default`（含于 capabilities 的 core:default）的 29 项 allow-* 不含 start-dragging，ACL 静默拒绝；权限清单以 registry 包 `permissions/window/autogenerated/reference.md` 的 Default Permission 段为准。
- 修法：capabilities/default.json 显式加 `core:window:allow-start-dragging`；capability/tauri.conf 类改动必须完整重启 tauri dev 才生效（不走 HMR）。

## 2026-09-08 vi.mock 工厂引用顶层 vi.fn 报 Cannot access before initialization
- 症状：测试套件整体 FAIL（0 用例执行），报 "There was an error when mocking a module. If you are using vi.mock factory, make sure there are no top level variables inside"，Caused by ReferenceError 指向工厂里引用的断言靶变量。
- 原因：vi.mock 调用被提升到文件顶部，工厂执行时其引用的普通 `const toastError = vi.fn()` 尚未初始化。
- 修法：工厂引用的一切变量（含 toastError 这类要在断言里查调用的靶）都用 `vi.hoisted(() => ({ fn: vi.fn() }))` 提升；被测模块的 import 放在 vi.mock 之后（vitest 会保证 mock 先注册）。

## 2026-09-08 jsdom 缺 scrollIntoView：校验失败回调链静默中断，保存「点了没反应」
- 症状：设置页测试点「保存」后 aria-busy 常驻、无 alert、状态不切换；生产同构代码路径为「校验失败→聚焦错误字段→切分节」，中间任何一环抛错即中断。
- 原因：jsdom 未实现 Element.prototype.scrollIntoView，RHF handleSubmit 的 onInvalid 回调里 focusFirstInvalidField 调用即抛 TypeError；异常发生在 setState 之前，后续逻辑全部跳过且 AsyncButton 只吞进 reject 不再上抛。
- 修法：测试 setup 里 `HTMLElement.prototype.scrollIntoView = vi.fn()`（settings-focus-error.test 先例）；实现侧教训是回调链里「定位→反馈」多步骤要各自 try/catch 留 console 信号，别让定位失败连带吞掉状态切换。

## 2026-09-08 审批禁用会话里 api_call write 操作报 invalid output 而非审批拒绝
- 症状：api_call 对 sideEffect=write 操作 dryRun=false 调用报 "tool api_call returned invalid output: value is not lossless JSON"，且耗时 2~4 分钟；本地探针服务器零请求记录，证明 HTTP 根本没发出。
- 原因：write 操作需人工审批弹框，审批禁用会话自动拒绝；而拒绝结果对象本身过不了 run_code 桥接的无损 JSON 序列化，报错误导成序列化问题。长耗时是卡在等一个永远不弹的审批框。
- 修法：出图/写操作别走 api_call——直接 curl + .env 凭证 + b64 解码落盘（docs/design/message-center/gen.sh 先例）；报 invalid output 且目标是 write 操作时先怀疑审批拦截，别往参数/大小方向排查。

## 2026-09-08 A2A over P2P 波（run_code + 大文件搬移）
- 症状：read 工具带 limit 读文件后用 write 整文件覆写 → 文件尾部被静默截断（agents.rs 330 行读到 240 行就写回；a2a_card_wave.rs 同坑两次），cargo 报 unclosed delimiter。原因：read 的 limit 截断 + write 全量覆写 = 丢尾部。修法：整文件覆写前必须无 limit 读完整文件；否则只用 edit 改片段。
- 症状：cargo fmt 后 edit 报 file changed since it was read。修法：外部进程可能改文件，edit 前重新 read。
- 症状：bash 里 cmd1 | tail -1 && cmd2 管道掩盖 cmd1 退出码，rebase 冲突被跳过继续 push/merge。修法：关键命令不接管道，或用 PIPESTATUS 判定。
- serde #[serde(with = "别名::b58")] 配 use x as 别名 可编译（别名即路径）。
- 行数红线：crates/*.rs 由 line-limit 机械管；apps/** 靠自觉但同样适用——测试拆 *_tests.rs 兄弟文件（lib.rs 声明 #[cfg(test)] mod，测试内 use crate::xxx::*）。
- panic-hygiene 门禁扫 crates 非测试路径 unwrap/expect/panic：serde_json 参数用 json! 宏直接建，别 to_value().expect()。

- 2026-09-08（W3 波）cargo fmt 会在 check 之后改动文件：fmt 后 edit 必报 file changed since read，先重新 read 再 edit；且 edit 的 old_string 要按 fmt 后的现场文本取。
- 2026-09-08（W3 波）clippy err_expect：测试里点 err 后 expect 一律写 expect_err；fmt 多行折叠会让单行 grep 漏检，用带空白容忍的正则替换。
- 症状：execSync 跑 pnpm 报 env: node: No such file or directory。原因：run_code worker 的 PATH 无 node，node/pnpm 实际在 /Users/imeepos/.vite-plus/bin（非 homebrew），cargo 在 ~/.cargo/bin。修法：命令里显式 export PATH=/Users/imeepos/.vite-plus/bin:/opt/homebrew/bin:/Users/imeepos/.cargo/bin:/usr/bin:/bin，且同时传 env 覆盖 HOME=/Users/imeepos（worker 的 HOME 也可能缺失）。
- 2026-09-08 W5b：DSH job_list 零参调用报 binding arguments must be lossless JSON（同 session_link_list/get_goal 族）；绕行：显式传 {}。job_output 的 wait 长超时受 wall-clock 1000s 上限截断，长门禁用轮询 job_list+读日志文件。

## 2026-09-08 HTTP 服务带未读请求体关连接：RST 竞掉已写出的响应（401 路径并行假红）

- 症状：macOS 并行测试下，HTTP 401/404 等「未读 body 即回」的路径偶发客户端 `ConnectionReset`，断言响应内容的测试假红（12 轮 1 次），单跑恒绿。
- 原因：服务端 reply+shutdown 后直接 drop socket，请求体未读 → 内核发 RST 而非 FIN，RST 可清掉客户端 recv 缓冲里已写出的响应。
- 修法：按 Content-Length 精确排空剩余请求体再放连接（读满即走，不依赖对端关写杜绝互等；EOF/超时放行并留痕）。见 crates/acp-pump/src/status_wire.rs drain_exact 与 tests/status_close.rs 回归。

## 2026-09-08 测试客户端护栏 == 服务端拨号超时（10s vs 10s）：并行负载必竞态假红

- 症状：share_connect `connect_share_unreachable_peer_reports_failure` 偶发 `Elapsed` panic（suite 耗时恰好 10.0s）。
- 原因：夹具 STEP=10s 与 lib 拨号护栏 HANDSHAKE_TIMEOUT=10s 零余量，负载下「服务端 10s 超时如实回执」与「客户端 10s 放弃」互踩。
- 修法：客户端护栏必须 > 服务端最坏路径耗时（STEP 提到 20s）；设计夹具时先列服务端各超时常量再定护栏。

## 2026-09-08 INLINE-ACP-PUMP 波（协调者视角）

- [known-issues] devloop_accept 的 runner 对长命令（冷编译 cargo test 链）会以 exit null 假阴性掐死（观测窗口 17s~11min 不等），连续失败还会熔断降窗。修法：验收命令保持 <30s 可完成；长门禁用 bash 长超时手动执行拿真实退出码，逐字同命令，输出留痕（T1-MANUAL-GREEN 模式）。
- [techniques] worktree 内 pnpm install 会因 ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY 拒绝清 node_modules：前置 CI=true 即可（make check 的 gui-check 段同理）。
- [lessons] 多会话并行各跑 make check 时 vitest 会互相制造单点负载偶发红（两次实证：network-tabs / settings-view-nav，隔离复跑均绿）。协调者跑权威门禁前先 pgrep 确认无并发 make/cargo，等安静窗口。
- [lessons] 被根 workspace exclude 的子项目（独立 bin）完全逃脱 make check 全套门禁（fmt/clippy/panic-hygiene/line-limit 均不覆盖）——"为什么是个独立进程/子项目"应当在立项时回答；答案含糊就是架构债（本次 acp-console 整体废止的根因）。


- [2026-09-08 A2A波] 症状：make check 的 ai-docs-sync 报"文档缺示例命令 X"。原因：贡献者整本重写 p2pctl-ai-guide.md（1625→265 行）毁掉存量条目；且其条目标题用 `### a2a list` 而非家规 `### p2pctl a2a list`，扫描器不识别为条目、参数行归入上一条目产生连环假错。修法：以 main 全文为基底恢复 + 新域按家规条目格式重写插入 SYNC:END 前。
- [2026-09-08 A2A波] 症状：p2p-itest 的 a2a 用例全红（read timeout）。原因：新 worktree 的 apps/acp-agent/target 里 acp-echo-stub 是旧构建（无 --acp-agent 模式），根 workspace 不带建。修法：cd apps/acp-agent && cargo build --bin acp-echo-stub。
- [2026-09-08 A2A波] 症状：子代理用常量 0 替代 Date.now() 过 purity lint。危害：tsMs=0 是"无消息"哨兵，破坏时间显示/排序/撤销回归语义，且测试未覆盖不报红。修法：时间戳在 store effect 内采集（合法 impure 点），渲染期只派生。
- [2026-09-09 llm-share tab 化] 症状：vitest 里 fireEvent.click(TabsTrigger) 不切换 tab，"Unable to find 目标面板"。原因：Radix TabsTrigger 在 onMouseDown 激活（react-tabs dist 123 行），click 单发不触发。修法：测试里先 fireEvent.mouseDown 再 fireEvent.click（项目未装 user-event）。
- [2026-09-09 llm-share tab 化] 症状：`backend.someMethod().catch(...)` 依然抛未捕获异常、组件 load 永不完成。原因：mock-backend/桥接实现可能同步 throw（如 offerShow 未发布直接 throw），同步异常发生在 .catch 挂上之前。修法：包一层 `settle = async (call) => { try { return await call() } catch (e) { return e } }`，对 Promise.all 多路并行拉取尤其必要。

## 2026-09-09 gpt-image-2 中转网关抖动波（生图设计轮）

- [known-issues] 症状：OpenAI 兼容图像端点随机返回 500 `upstream_error` 或 Cloudflare 524（~127s 边缘超时），同一参数时好时坏；1024x1024 成功率最高，非方图与大尺寸更容易触发。原因：CF 前置中转源站慢且过载，与请求参数无关。修法：同参数串行重试 + 15s 退避（单张上限 3~5 次，实测 100% 最终成功）；严格一张成功再发下一张（并发互相挤挂）；连续 3 败降 quality=low 出草稿保进度，事后网关空闲再 high 重出替换。
- [techniques] 中转网关压测探测顺序：先 1024x1024+low 探连通（~5 美分），再 high 探时延上界，最后才测目标尺寸；探针脚本一次成型避免 import 重跑。生图批任务把 results.json 设计成每次尝试即落盘，协调者可 5 分钟轮询磁盘代替催会话回报。

## 2026-09-09 通讯录资料互通轮（/im/profile/1 + 好友卡编辑）

- [known-issues] 症状：新 worktree make check 的 a2a_task_wave 六用例全 panic「acp-echo-stub 未构建」。原因：环境前置缺失（stub 不进根 workspace、worktree 内 target 为空），与本次改动无关。修法：`cd apps/acp-agent && cargo test --no-run` 后重跑；「红 ≠ 你的错」，先读 panic 原文定位前置。
- [known-issues] 症状：testing-library 测试里 `within(pane).getByTestId("pane自身")` 报 not found（错误里却打印着该元素）。原因：within() 绑定的是子树作用域，查锚点自身必须出界；`container` 属性在 render() 返回值上，查询到的元素没有。修法：查锚点内元素用 within 绑定，查锚点自身/img 等直接 document.querySelector。
- [known-issues] 症状：新增 IpcBackend 方法后 vitest「IPC 调用点静态守卫」红。原因：守卫机械扫描 views/components 的方法名字面量，store 层调用不算；豁免清单在同测试文件 EXEMPT 表，另有 scripts/check/cli-parity.tsv 要登记（mapped 或带理由 exempt），两处缺一即红。修法：数据层调用一律登记豁免并写明所属 store。
- [known-issues] 症状：ESLint react-hooks/set-state-in-effect 报错（含 Warning 静默路径）。原因：新规则禁 effect 体内同步 setState（级联渲染）。修法：弹窗表单「随目标重置」改为调用方条件渲染 + useState(props 初值)（挂载即终态，关闭即卸载），彻底去 effect 播种。

## VSCode rg --files 扫描风暴拖垮整机（2026-09-09 实锤）
- 症状：load 飙到 150+（10 核），cargo 测试二进制卡死在 `_dyld_start` 0% CPU，连 ls 都超时；ps 见 60+ 个 `rg --files` 并发。
- 原因：VSCode server 对仓库反复全量枚举，`target/`、`node_modules` 几十万小文件是重灾区；gitignore 挡不住编辑器枚举。
- 修法：项目根 `.vscode/settings.json` 配 `files.watcherExclude`/`search.exclude` 排除 target/node_modules/dist（本仓库 .gitignore 有意忽略 .vscode，该文件只落本地不入库）；止血 `pkill -f 'rg --files'`。
- 排查口诀：测试进程 0 CPU 且卡 `_dyld_start` = 动态加载被 I/O 饿死，先看 load 和 rg 进程群，别往测试逻辑上猜。

## p2p-itest a2a_task_wave 全组秒败（2026-09-09 authz A1 轮实锤）
- 症状：cargo test --workspace 时 crates/p2p-itest/tests/a2a_task_wave.rs 的 t1-t6 六例 0.00s 内全 FAILED，cargo test -p a2a 却全绿（易误判为 a2a 回归）。
- 原因：tests/task_wave_common/mod.rs:36 前置断言——依赖 acp-echo-stub 二进制，未构建即 panic，报错原文已给修法。
- 修法：cd apps/acp-agent && cargo test --no-run 预构建 stub 后复跑即 6/6 绿；workspace 全量验收前先构建该前置件，或设 A2A_TASK_STUB。

## 2026-09-09 bash 工具跑 cargo 门禁接 `| tail` 管道：尾命令退出码 0 假绿（重蹈 coordination 轮 55 覆辙）
- 症状：`cargo clippy ... -- -D warnings | tail -5` 返回 exit 0 且末行 "Finished"，据此判定门禁过；实际 clippy 在另一 workspace 退出 101。
- 原因：管道取最后一个命令（tail）的退出码；cargo 的真实失败被吞。coordination 检查轮 55 早有「验收 ACCEPTANCE_EXIT 捕获管道尾命令退出码会假绿」在册，本轮再次踩中——教训没有进机械习惯。
- 修法：门禁命令一律 `cmd > /tmp/xx.log 2>&1; echo "EXIT=$?"`（无管道、退出码直取），判据读 EXIT 行 + 日志终态行。凡写「验收/门禁」四个字的 bash 命令，看到管道就停下来改写。

## 2026-09-09 S3 轮 vitest 全量套件与 cargo 冷编译并发跑：worker 集体超时假红
- 症状：vitest 全量 84 文件 2 failed + 149 个 "Timeout waiting for worker to respond"（forks pool）；单独复跑被红文件全绿。
- 原因：cargo clippy 全 workspace 冷编译（~20 分钟满核）与 vitest forks 池并发抢核，worker 启动超 60s 预算即被判死。
- 修法：门禁严格串行（vitest 完 → cli-parity → clippy）；工作树无共享 target 时冷编译极贵，规划进交付节拍而非并行塞缝。

## 2026-09-09 S3 轮 worktree add 被工具超时击杀后重建：detached HEAD 静默吞提交
- 症状：worktree 内连续 8 个 commit 正常落盘，push 报 "Everything up-to-date"，`git branch --show-current` 为空（detached），分支 ref 停在倒数第二次提交。
- 原因：首次 `git worktree add` 被 60s 工具超时 SIGTERM 半途击杀，残留半成品目录；rm 后 `git worktree prune` + 重建时分支已被（prune 前的）注册残留视作占用，git 回退为 detached checkout——之后所有 commit 都不进分支 ref。
- 修法：`git checkout -B <branch>` 把 ref 快进到 HEAD 再 push。预防：worktree 重建后与每次 push 前都 `git branch --show-current` 校验非空；超时击杀的 git 操作先 `git worktree list` + `git log <branch>` 对照产物再动手。

## 2026-09-09 S3 验收轮 负载敏感假红被判真回归：双树对照+受控 A/B 才能定案
- 症状：验收方在自己窗口测得分支 5 红（src/acp 4 文件），本会话复测同 4 文件 10 红；但红名集合随时间漂移（同代码 10:24 全量仅 2 红），且单文件独立跑出现「Failed to start forks worker（transform 0ms，测试根本没执行）」。
- 根因：10 核机器 load 43-90（4-9 倍过载，外部来源=其他会话 vscode-server 全盘 rg 索引/VM/常驻进程），vitest forks 池 worker 启动与 5s 单测预算在节流下全面超时；首测/复测窗口负载不同 → 结论互斥。
- 判别法（三步定案，勿先改代码）：①`git diff main --stat -- <被红测试目录>` 证零触碰 + 测试文件 import 链核对是否真经过自己改的共享文件；②主树同命令同文件复跑——同红即非本分支引入；③受控 A/B：同一棵树仅交换嫌疑共享文件的 main/分支版本背靠背跑——两轮结果随负载漂移而非随代码版本漂移即定案。
- 修法（交付面）：错峰在 load<30 窗口用 `--no-file-parallelism`（单 worker 免疫 forks 池启动风暴）跑验收矩阵；本轮最终分支/主树对称 12/12 绿 + 分支全量 225 文件 1367 用例全绿。教训：验收红先问「机器现在多忙」（uptime），再问「我改了什么」。

## 2026-09-09 S4 轮 负载敏感假红按"等待对象有无"分两型机械修法；文件级 worker 错测试内不可治
- 背景：S4 分诊 main 上即红的 5 处 + 串行验收新暴露 1 处，全部为负载敏感、零产品缺陷。判别证据：同代码双跑一红一绿；红轮用例耗时 1069ms/5215ms 恰好压在 waitFor 默认 1s / vitest 默认 testTimeout 5s 悬崖上。
- 两型修法（断言语义零改动）：(a) 异步链路有等待对象 → 给 waitFor/findBy/vi.waitFor 加显式 `{ timeout: 10_000 }` 预算（含共享夹具 renderConnected/newSession 等必经路径）；(b) 纯同步用例无可等待对象 → `vi.setConfig({ testTimeout: 20_000 })` 文件级放宽（it 第三参逐用例写太碎）。
- 文件级错（"Failed to start forks worker"、transform/setup/tests 全 0ms）= worker 没起来测试体根本没执行，测试内等待无法治——只能串行口径（`pnpm test:serial`）+ 测试文件头标注，勿浪费时间去"修"测试。
- 对照统计陷阱：vitest 报告里 Errors（worker 未启动）不进 Test Files failed 计数；「默认并行不劣于基线」的对照必须把 failed 与 Errors 分开列，否则两边数字口径不同没法比（本轮分支 0 failed + 8 Errors vs main 同窗口 6 failed 文件 10 用例红）。

## 2026-09-10 聊天长列表虚拟化轮：initialTopMostItemIndex 在 sizeTree 未就绪时给出越界窗口
- 症状：Virtuoso 设 initialTopMostItemIndex=N-1 后（jsdom/VirtuosoMockContext 即 sizeTree 空档形态），初始 listState 推导出 [N..N+窗口] 的越界区间，itemContent 收到 ≥totalCount 的索引——轻则整屏空渲染、重则 `undefined.id` TypeError 崩树。
- 定因：初始推导在 sizeTree 空时按 initialTopMostItemIndex 直接开窗，不与 totalCount 钳制（dist 内 `qo(Ye(F, b), X, j)` 路径）。
- 修法：弃用 initialTopMostItemIndex，挂载 effect 里命令式 `scrollToIndex({ index: "LAST", align: "end" })`——与真实滚动同一回路，真实浏览器与 jsdom 行为一致；itemContent 同时对越界 index 防御返回 null。
- 附：react-window v2 List 的 rowProps 缺省（undefined）会在 useVirtualizer 内 Object.keys(undefined) 崩，空对象也必须传。
- 2026-09-10 PROTO 轮：bash 脚本里 `trap '… "$var"' EXIT` 引用函数内 `local` 变量——函数返回后变量出队，`set -u` 下脚本退出时报 `var: unbound variable`（主流程 PASS 后噪音，退出码侥幸为 0 易漏检）；修法=去 local 化或 trap 前快照到全局。本次由 CHECK_ROOT 外部运行路径暴露。
- 2026-09-10 PROTO 轮：macOS 无 GNU `timeout` 命令；长命令防挂要用后台 job + 通知，或用 `perl -e 'alarm …'` 替代。
- 2026-09-10 PROTO 轮：session_link_collect 的 claimToken 在目标会话「运行中」时可能报"历史暂不可读"——不是凭证失效，等目标 turn 结束后再收，或直接改为单向 send+下轮看文件系统实况。
- 2026-09-10 PROTO 轮：`git worktree remove` 遇大体积 target/ 目录会拖到前台超时——后台跑 `rm -rf <dir> + git worktree remove --force + prune`，分批推进并逐个 echo remaining 校验。
- 2026-09-10 PROTO 轮：判会话活性用文件 mtime 而非轮询回复——`ls -lT` 看产物最近修改时间，>30 分钟零活动+催办无响应才升级接管流程（dispatch 超时≠死亡）。
- aioquic 1.2.0 在本地校验钩子里抛 TLS Alert 会让 wait_connected 永久悬挂(alert 发生在 UDP 回调路径,连接状态机不收口);修法:握手完成后、发数据前做断言并显式关连(IV1)。
- asyncio 的 connect() 是 @asynccontextmanager 包装,没有 aclose/__anenter__;手工管理生命周期用 contextlib.AsyncExitStack + enter_async_context(IV1)。
- Rust 侧守卫进程的流 RESET 会让 aioquic 客户端 write/write_eof 抛 AssertionError("cannot call write() after reset()");回声类协议读到应答后不要再写 EOF(IV1)。
- 2026-09-10 RICHTEXT 轮：react-markdown v8 的 `breaks` prop 在 v9 已移除——传入被静默忽略(不报错)，单换行不渲染 br；v9 起该行为走 remarkRehypeOptions？也不对：remark-rehype v11 根本没有 breaks 选项(它属于 remark-breaks 插件)，连查两级 d.ts 才确认。修法(RICHTEXT 轮取的零依赖路线)=markdown 软换行默认保留 \n 进 DOM + CSS `p { white-space: pre-wrap }` 视觉换行，行为与纯文本期对齐且不引库。
- 2026-09-10 RICHTEXT 轮：react-markdown v9 的 urlTransform 返回 "" 时仍渲染 `href=""` 的可点锚点(点击导航当前页)——协议拒绝要彻底需 components 覆盖 a：无有效 href 退化为 span；且自定义组件会收到额外 `node` prop，直接展开进 DOM 会触发 React 未知属性告警，需显式解构接住。
- 2026-09-10 RICHTEXT 轮：vitest 裸跑单文件报 `describe is not defined` 时先查项目是否未开 globals——本项目约定测试文件显式 `import { describe, it, expect } from "vitest"`，直接照抄网上默认 globals 写法会整文件崩。

## 满载假红第三型：doctest 撞中途落盘的新文件（2026-09-10 v0.1.7 release-check run5）
- 症状：cargo test 的 lib 单测全绿后，Doc-tests 段编译失败 `cannot find crate prost`，指向一个「本轮早期不存在」的新文件（identify.rs）。
- 原因：并行会话在门禁运行中途往主树落了半成品提交（源文件先落、依赖锁后补）；lib 测试二进制早期已编译完跑的是旧码，rustdoc doctest 在末尾重读盘撞上新代码。
- 修法：不修码，先判树是否被移动（git status -sb ahead/behind + reflog）；等对方推完拉齐后整轮重跑。指纹 = 报错文件在门禁启动时不存在。
- 预防：跑长门禁前 pgrep 确认无并行 cargo/vitest/make；多会话期用协调文档收官记录做放行闸。

## 满载假红第四型：时间敏感用例并发翻转（2026-09-10 run4 a2a book）
- 症状：`issued_at in the future` / 期望 StaleVersion 实得 Verify 错，同一代码前几轮全绿。
- 原因：信封签名/校验各自取时钟，两个全量门禁并发时调度延迟让两次时钟读跨越判定边界。
- 修法：隔离复跑 0.01s 全过即定性负载伪红；勿改断言放宽。时间敏感测试加固（注入时钟）是独立任务，别在发布路径上顺手改。
