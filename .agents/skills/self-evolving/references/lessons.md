# Lessons

<!-- 一条经验一行。格式：当 X 发生时，修复是 Y。skill 没提前警告我。 -->

_none yet — be the first._
- 2026-09-05：任务书 scope 行与交付 goal 行矛盾（如债3 明写「facade 装配期」却限「只动三 crate」）时，修复是按 goal 实现 + 每个越线文件列入 commit 正文 + 提交后验收前向协调方单列「范围说明，请裁决」——先斩后奏但全程透明，比卡死等裁决或隐瞒改动都省轮次（BASE1 获「放行」且论证被要求保留进汇报）。
- 2026-09-07：当「客户端测试全绿 + 服务端测试全绿 + 真机必挂」同时成立时，修复是找两层测试的传输层盲区（CORS 预检/Origin/协议升级）——stub 掉 fetch 的前端测试和裸 TCP 的后端测试各自证明不了「浏览器对真服务」这段路，跨进程契约要专门补真机 E2E 或模拟对端传输语义的用例。
- 2026-09-04：当要在带机械校验区间的文档（如 p2pctl-ai-guide 的 AI-DOCS-SYNC 区间）回填内容时，修复是区间内只加纯文字、新章节放区间标记外——区间正则把散文里的 `--词`/`<大写>`/`[大写]`（连 `[CAPTURE_PERMISSION_DENIED]` 这种错误码）都抽成参数与 `--help` 双向比对，多一个 token 整个门禁红。
- 2026-09-05：Tauri app.emit 在前端 listen 装好之前发出即永久丢失（事件不缓存），E2E 断言"前端已感知"前必须先等就绪门（监听装好后向前端日志写标记行，脚本轮询到标记再开写）。
- 2026-09-05：集成不熟的依赖库先 cargo fetch + grep registry 源码确认真实签名再写码，别凭文档记忆写完再修（notify-debouncer-mini 0.5 三个假设全错：new_debouncer 只有 2 参、DebouncedEvent.path 是单数、Debouncer<T> 泛型是 Watcher 不是 handler）。
- 2026-09-05：任务卡的"触及路径白名单"要与需求联动自查后再动手：给中央注册表加条目必然改它的守卫测试（page-registry.test.ts 路由数清单 8→9），白名单漏列守卫测试时唯一解是「最小修改 + 回报显式标记例外」，硬守白名单只会让门禁假红、A1 必挂。
- 2026-09-05：run_code 用模板串写长文件时，漏闭合反引号/转义混乱会在"解析 program"阶段就炸（Expected ',' got ';' / Unterminated template），与目标文件内容无关；先写 30 行小片验证模板串本身，再写全文件。
- 2026-09-05：工具类管道命令 `make check 2>&1 | tail` 的 exit code 是 tail 的（恒 0），make 失败被吞；一律 `set -o pipefail` 或 `rc=$?; echo RC=$rc` 显式回传（验证两次才敢报绿）。
- 2026-09-05：pnpm test -- --run <path> 对 script 形 vitest 并不能按路径过滤（全量照跑），子集调试用 `pnpm vitest run <path>`。_
- 2026-09-08：本仓 TSX 代码行（含行尾注释）出现 CJK 即被 i18n hardcoded-copy 门禁拦下，注释独占一行才豁免；中文注释一律独立成行，绝不作行尾注释。
- 2026-09-08：用本仓封装 API（如 toastSuccess）前先读封装签名——它是 sonner 的薄封装但二参退化为 string，按底层库签名写 `{ description }` 会被 tsc 拦。
- 2026-09-08：开工前先在基线上跑一次 clippy/门禁再动手：rust-toolchain 锁 1.98.1 引入 items_after_test_module 新 lint 时 main 本身已红，不先验证会把存量红误判为自己改坏（存量红独立 chore 提交，不混进 feature）。
- 2026-09-08：移除 GUI 功能前先 grep 它是不是某 IPC 方法的唯一界面调用点——是则 IPC 调用点守卫测试（chat-friend-add.test.tsx EXEMPT 清单）必红；契约保留后端/CLI 用的方法登记豁免并写明原因，连后端一起删才走契约修订。
- 2026-09-03：GUI types/node_event.rs 对 NodeEvent 无通配符穷举匹配，swarm 侧新增事件变体必须走 LifecycleEvent 独立通道加法（E6/E8 两次先例），加变体前先 grep 全部 recv 点匹配严格度。
- 2026-09-03：run_code 写 Rust 代码文件时，JS 双引号串会被 Rust 内嵌双引号截断（Expected ',' got 'ident'）；全部行改用 JS 单引号串（Rust 源内几乎无单引号字符），且整文件构建+写入必须在同一次 run_code 调用内完成（跨调用无内存）。
- 2026-09-03：会话宿主重启丢工作区后，靠协调者 wip 检查点提交 + 承接会话「审阅→补全→补 itest→过验收」流程恢复；恢复后先 git log/git status 对账，不盲信记忆中的文件状态。
- 2026-09-03：CI workflow 文件存在不等于门禁生效；先按实际 git remote 与事件触发矩阵核对，主线直推、PR、tag 三条路径都必须有可执行 gate。
- 2026-09-03：门禁脚本本身也必须被成功/失败夹具覆盖并纳入聚合门禁，否则新增检查可能悄悄退化成假绿。
- 2026-09-01 harness 的 edit 工具按文件路径记账：主树读过的文件，在 worktree 下的同名文件仍需先 read 再 edit，否则直接拒绝。
- 2026-09-01 未来/新版本工具链（rustc 1.98）的 std API 可能与训练记忆不符，编译错误时先读编译器诊断本身（它会给新签名提示），别按旧 API 硬猜。

- 2026-09-02：跨语言脚本里用 python/ruby 批量做文本替换时，replace 未命中会静默成功返回——改数与改后内容必须断言或复查；Rust 场景优先用结构化 edit 工具按精确 old_string 改。
- 2026-09-02：并发驱动任务"等待外部请求"与"推进内部 IO"必须放在同一个 select 中，任何一处单独 await 都可能饿死另一方（yamux 驱动实证）。
- 2026-09-02：封装库把上游 poll API 包成 AsyncRead/AsyncWrite 时，读写长度必须以调用方缓冲为上限，不得自定常量（quinn 适配器实证）。
- 2026-09-02：tokio::join! 的两个 future 各自可变借用同一变量必 E0499；小缓冲管道（duplex 4KB）下两端同时 write 还会互堵死锁。测试按阶段"一写一读并发"设计，别追求全双工对称。
- 2026-09-02：expect_err/unwrap 要求 Ok 类型实现 Debug；Box<dyn Trait> 没有 Debug，改用 match 取 Err 并 panic，别给协议类型强加 Debug。
- 2026-09-02：停车式协议（首个请求挂起等配对方）必须保证每个请求帧最终都有响应帧（成功 Bound 或显式 Reject），否则客户端只能靠超时兜底，等于没有错误信号；Bound 应在配对成功时同时发给两侧。
- 2026-09-02：客户端读回包遇 EOF（对端干净关流）要映射为显式 LinkClosed，落到"意外回包"分支会把排查引向协议层假问题。
- 2026-09-02：验收测试与构建会改写 Cargo.lock，收尾 remove worktree 前先 diff 锁文件；漂移仅多版本规范化时 checkout -- 丢弃即可。
- 2026-09-02：run_code 里多行 bash 命令放进 "..." 双引号字符串会因换行直接 JS 语法错误（Expected ',', got 'ident'）；用数组 join(" && ") 拼接，长 commit message 用 heredoc（git commit -F - <<'MSG'）。
- 2026-09-02：文档与代码对齐任务先 grep 常量与结构再落笔，别信设计稿——p2p design §6 写 sha256(protobuf(公钥))，实现是原始 32 字节公钥哈希（p2p-identity lib.rs）。
- 2026-09-05：协调多会话并行改同一 GUI 域，按文件所有权切分范围 + i18n 中央登记按键块分治（各持专属键块 append-only）+ 跨会话依赖在增补单里显式指定归属与契约值（如错误结算约定 stopReason="error" 由 A 产 B 渲染），全程零冲突；验收在主树由负责人机械复跑而非采信会话自报。

- 2026-09-02：glob "crates/*/src/*.rs" 只匹配单层目录，mod 子目录（如 rendezvous/）会漏；枚举 crate 源码用 "crates/**\/*.rs" 再读，避免误判"文件不存在"。
- 2026-09-02：要从文件里抽出保留段时，先从原 ref（git show HEAD:path）抽出，再覆写文件；先覆写再从新文件 sed 抽段，抽出来必然是空段或错段（X 会话拆测试子模块踩过，靠 git show HEAD 无损重做）。
- 2026-09-02：交接描述里"当前 main 是绿的"要用门禁实测复核，别照抄；fmt 门禁上线即暴露 32 文件存量违规，红的是存量不是门禁。
- 2026-09-02：tokio::join!(a, b) 求值即等待、返回 tuple，不是 Future；直接塞给 timeout/自定义 expect_within 报 E0277 "tuple is not a future"，包一层 async { tokio::join!(..) } 即可。
- 2026-09-02：clippy never_loop：loop 体每个分支都 return/panic 时删掉 loop 直接 match；若本意是"跳过不合意事件继续等"，就把该分支写成显式 continue，别让每个分支都退出。
- 2026-09-02：并行会话进行中 main 会新增门禁（本日中途上了 make check）；收尾验收跑仓库门禁本身（make check），别只跑任务书里写死的三条命令，且验收前再 rebase 一次 main。
- 2026-09-02：挂死诊断三板斧实测有效：ps 找测试进程 pid → macOS sample <pid> 2 出线程栈 → 看到 kevent+park 即"全任务 pending 等 IO"，逐个检查 duplex EOF 传播链（见 known-issues split BiLock 条）。
- 2026-09-02：改任何 pub API 签名后立刻全仓 grep 调用点（含 crates/*-itest 这类集成测试消费者）——workspace 级 make check 会编译一切，跨 crate 断裂在最外层门禁才爆，返工面大。
- 2026-09-02：安全审查修复轮动线协议语义（如接入白名单）时，先全仓找断言旧语义的测试：它们不仅是编译断裂，更是语义预期冲突，要显式决定"改测试对齐新语义"还是"保留兼容入口"，并在报告中声明边界偏差。
- 2026-09-02：协调派单的"只改自己 crate"与"make check 全绿"冲突时（他人测试消费我的 API），机械适配对方的测试装置属验收必需，零生产代码改动、逐文件显式 add、报告中声明即可；切勿为了守边界让门禁保持红。
- 2026-09-02：集成冒烟在共享网络环境跑时发现面是全局的——局域网里其他会话/机器的同类节点会经 mDNS 涌进地址簿（含不可达 fe80/240e 地址），把确定性测试打崩；给 CLI 留「收窄发现面」的开关（如 --no-mdns 只走 rendezvous）是正当功能而非 hack，冒烟走确定性路径。
- 2026-09-02：间歇性测试失败先抓「环境差分」再查代码：对比通过/失败两轮的输出差异（对端数量、地址列表长度、端口占用者），本轮失败轮发现 peer 数 4 vs 通过轮 2，直接指向 LAN 噪声而非自身 bug。
- 2026-09-03：拆分多个 commit 前，工作树可能已有预暂存文件（status 首列 M）——git add <paths> && git commit 会把全部暂存内容打进第一个 commit；每个 commit 后立刻 git log --oneline 复核对数，"nothing to commit"（exit≠0）会让 && 链静默跳过且空输出切片看不出异常。
- 2026-09-03：mock 后端的语义要与真实状态机对齐，不是"返回合理数据"就行——mock peerDisconnect 最初按 knownPeers.includes 返回 true，幂等挂断测试立刻打脸；给 mock 显式建 connectedPeers 状态集，断开/重连/挂断全走它。
- 2026-09-03：往 290+ 行的文件加功能前先主动开新模块（swarm/hangup.rs 先例），别等 line-limit 红了再拆；make fmt 会改动行数统计，line-limit 评审以 fmt 之后的行数为准。
- 2026-09-03：edit 的 read 记账是按绝对路径的（2026-09-01 旧教训的补充）：bash cat 读过、或读过主树同名文件，都不算数；worktree 下编辑前必须对同一路径先 read。
- 2026-09-02：多树并行时验证结论锚定"产物 mtime/版本"，不锚定命令输出——cargo build 换了目录照样 Finished，跑的却是另一棵树的旧二进制；冒烟前 ls -la 产物核对时间戳，一行省一轮返工（R-E4 smoke3 假阴性实录）。
- 2026-09-02：写"释放/回收"类服务端记账修复时，单押一个触发信号（流级 EOF）在真实环境必然漏（读半被任务钉住、半开连接、EOF 传播链断裂）；成对设计——精确信号（流关闭）+ 粗粒度兜底（链路归零），且粗粒度触发器要在 mock itest 里可复现，否则回归只验了理想路径。
- 2026-09-02：rustfmt 会把"紧凑压行"的测试数组展开回标准样式（285→326 行），压行数不能对抗 fmt——结构性迁移到 tests/*.rs 集成测试（lib.rs pub mod 已导出即可用）才是正解。
- 2026-09-02：管道 `make check | tail` 后取 $? 拿到的是 tail 的退出码，门禁假绿——必须 `make check > log 2>&1; echo $?` 直取 make 退出码。
- 2026-09-02：定位"间歇性失败"先设计最小干净实验把偶发定性为必然（全新服务端+单客户端测控制流寿命，31/31 次 ~90ms 死亡），再读常数推演（32 槽配额/TTL 3600s/5s churn ⇒ 稳态负载 60>32 必自锁）——一轮定量实验胜过十轮复测。
- 2026-09-02：清理长驻测试进程用 kill -9 + lsof 按端口反查（普通 kill 对刚启动的 tokio 进程有竞态，残留进程占端口会让后续轮次报 Address already in use，与真实 bug 混淆）。
- 2026-09-02：新环境首跑报错先做「基线对照」再定性：同一栈在旧节点（138）与新车（ECS）行为一致与否，一条对照日志就把「部署缺陷」改判成「系统常态」或锁定真差异——单点异常不等于新车问题。
- 2026-09-02：「跨公网才复现」不等于环境问题——可能只是该路径首次被真正执行。/t3401 断链在整段无分段的 localhost 管道下照挂，跨公网只是首个真实跑 TCP rendezvous 的场景；复现后第一步先跑「最朴素条件」对照，再谈 MTU/时序。
- 2026-09-02：QUIC 正常、TCP 挂的排查顺序：先对比两条路径的**生命周期语义**（连接谁持有、句柄丢弃意味着什么），再查帧格式/加密实现——quinn 驱动任务持有连接 vs YamuxMux 句柄归零即断链，语义差异直接就是根因。
- 2026-09-02：用 JS 写包含 shell `${}`、反引号或嵌套引号的大文件时，避免 JS 模板串；优先普通字符串数组逐行 join，写入后立即 bash -n/self-check，减少转义错误的连环返工。
- 2026-09-02：委派脚本任务要把每条验收条件映射成可执行命令并检查回报清单；即使实现者声称完成，缺少一项 self-check 也必须拒收后补齐。
- 2026-09-02：静态读码找并发/时序 bug 收益极低（本次 30 分钟白读 NoiseStream 三遍），先做分层消融实验（去 Noise、去 yamux、去抖动、去分段）+ 句柄对照（mem::forget 验证生命周期假设），5 分钟内收敛到唯一变量。
- 2026-09-02：AGENTS.md/文档声称的远端名不保证适用当前仓库——任何 fetch/push 前先 `git remote -v` 实测（p2p 仓库只有 origin 无 gitea，按文档 `git fetch gitea` 直接 fatal）。
- 2026-09-02 E5：关键路径的实现型派单要慎用——设计已在手时，子代理 10 分钟无文件产出即是接管信号，自己动手比等更可靠；开放式探索才值得委派。判断依据：dispatch 后定期 `git status` 看落盘进度，零落盘即收回。
- 2026-09-02 E5：审计子代理跑在仓库快照上时，其报告的「基线 commit」必须与自己的提交序列对表读——并行提交会让它的行号/文件清单快速过期，读报告先看 git log 再采纳。
- 2026-09-02 G-A：冻结契约里的注释示例可能与权威解析器不一致（gui-contract §3 注释写 "1.2.3.4/3400"，内核 parse_transport_addr/TransportAddr 展示强制 u/t 前缀）——实现对齐**可执行的权威源**（内核代码 + 后出的澄清章节），文档偏差上报协调方修订，不自行迁就注释。
- 2026-09-02 G-A：在宿主语言字符串里嵌另一语言代码（JS 模板串塞 Rust 文件）时，一处语法错整个脚本静默不执行且报错点远离病灶——大文件写入拆成多个小步骤、每步一个文件，失败面小、易定位。
- 2026-09-02 G-A：多文件超行数红线时用 hunk 级暂存（git apply --cached 选块）把「行为修复」与「纯结构拆分」拆成独立可 revert 的提交，避免为行数纠缠成一锅端。
- 2026-09-02 G-A：验收门禁（cargo test）先于人工审查抓真 bug——serde camelCase 缺失、父目录缺失、地址语法偏差全是单测抓的；跑门禁不要等"写完所有再跑"，骨架提交后立即预热构建能把编译错误提前 10 分钟暴露。
- 2026-09-02 gui-shell：构建验证与 git 提交必须分开两条命令跑；串在一条里
  （分号链）会让 build 失败后提交照常落盘，事后只能 amend。
- 2026-09-02 gui-shell：run_code 模板字符串会吃正则反斜杠（/\d+/ 写入成 /d+/），
  写正则用 new RegExp 字符串形式或双写反斜杠。
- 2026-09-02 gui-views-config：react-hook-form useFieldArray 仅支持对象行数组，
  string[] 字段的 FieldArrayPath 解析为 never（TS2322 string not assignable to
  never）；表单数组字段一律用 { value: string }[] 行模型，出入做双向转换。
- 2026-09-02 gui-views-config：拆"函数 ≤60 行"要拆 JSX 组件本体而不是抽 hook——
  把弹框内容抽成展示子组件（state 留父组件）一次就能从 126 行降到 ≤60。
- 2026-09-02 gui-views-config：协调者裁决——routes 薄挂载这类注册类接线变更要压独立小提交，
  不得混进 feat 大提交（与 menu.def/i18n 同类）。
- 2026-09-02 gui-views-config：GUI 派单前先查底座 facade 可达性——pub(crate) 能力（如 rendezvous
  手动注册/查询）GUI 不改 crates 无法接通，按钮只能置灰/移除加说明，避免做出假反馈 UI。
- 2026-09-02 gui-views-monitor：ask_user_question 有 600s 墙钟上限，协调者未应答会整体超时报错且拿不到答案——派单类会话把「范围冲突 + 推荐方案」一次性发出，超时后按推荐默认继续并在回报中标注待追认，不要原地等第二轮。
- 2026-09-02 gui-views-monitor：骨架能力缺口优先找「不改骨架」的等价通道：zustand store 的公共 setState（清空事件）、subscribe 回调里 WeakMap 盖接收时间戳（事件 tsMs 兜底）都能把需求收进视图层；确需改骨架的做成独立一行提交并显式标注待追认。
- 2026-09-02 gui-views-monitor：机械验收别目测——函数行数用括号匹配脚本量（注意先括号匹配参数再取函数体，组件解构参数的 { } 会被朴素匹配误当函数体），i18n 中英集合用 esbuild 转译成 CJS 后 require 比对叶子 key，一次脚本跑完比逐个眼查快且零漏。
- 2026-09-03 白屏复盘：门禁绿不等于应用能启动。静态检查（tsc/clippy/fmt）与单元测试对整应用渲染期崩溃零覆盖，必须有一条真实挂载整应用入口的启动冒烟测试；该冒烟第一天就抓到两个真雷（__APP_VERSION__ define 缺失、selectPeerList 快照漂移）。
- 2026-09-03：ErrorBoundary 是兜底 UI 不是修复——渲染循环类崩溃被兜住后用户看到的是「界面出错了」页，应用照样不可用；根因要修本体，兜底只是让失败可见。
- 2026-09-03：跨层灾难要同时修发现路径和故障点：故障在渲染层，失误在门禁层（GUI 零门禁），防复发的机制修复落在 Makefile 门禁，不是改完渲染代码就完事。
- 2026-09-03 G-H：注册拆分规则以「可回滚可审阅」为本意，硬约束优先于形式——i18n 键与视图拆开必红 tsc 时，标准做法是先提 locale 键（含未消费键）独立小提交再提视图（协调者终裁追认）。

- 2026-09-03 G-U1：依赖升大版本会重命名 feature（reqwest 0.13 把 rustls-tls 改成 rustls，TLS 实现内部化为 __rustls）——按记忆写 feature 直接 resolution 失败，报错信息列出的 available features 就是权威清单，加依赖前别背旧名。
- 2026-09-03 G-U1：并行会话高速推进时"反向同步"是循环不是动作——合并+门禁期间 main 还会前进，收尾用 git merge-base --is-ancestor main HEAD 机械判定（exit 0 才算同步），非零就再 merge 一轮；docs-only 增量的门禁增量重跑只要几秒，别省。
- 2026-09-03：serde 字段级默认只兜 JSON 字段缺失，不兜显式 []——旧版本/清空动作落盘的空列表会让「出厂默认」永久失效；「空回落默认」必须在装配/读取层显式实现（state.rs with_factory_fallback），且 UI 文案承诺的行为（空态提示）要在后端有对应代码，文案即契约需机械验证而非口头对齐。
- 2026-09-03：独立 [workspace] 子 crate（src-tauri）不在根 workspace 的 fmt 门禁覆盖内，提交格式不受 rustfmt 约束——在其中跑 cargo fmt 会重排大量无关已提交代码污染提交；fmt 只在门禁覆盖范围跑，churn 一律 git checkout -- 回退，提交前 git diff --stat 核对改动文件清单与本次任务严丝合缝。
- 2026-09-03：多会话仓库收尾竞态的实操压缩：rebase main 与 ff-only 合并放进同一条 bash 命令原子执行（本次两轮 ff 失败均因 main 在门禁窗口前进；docs-only 增量核 diff --stat 零重叠即可免重跑门禁，代码增量仍需复跑）。
- 2026-09-03：dev mock 后端的「默认值」与真实后端默认也要同源镜像，不只行为语义——mock DEFAULT_CONFIG 全空列表让 dev 页面呈现与生产开箱态完全不同，排查时会被引向不存在的配置 bug；镜像常量从单一源 import，别抄字面量。
- 2026-09-02：run_code 单个工具调用参数拼写错误会让整个程序体编译失败，其前面已排队的 edit/写入全部未执行——失败后必须核对哪些调用真的生效了，不能凭顺序假设。
- 2026-09-02：bash 调用忘带 workdir 时 npx 会从注册表拉最新版工具（vite@8）而非本地 bin；启动子进程一律用 ./node_modules/.bin/xxx 加显式 workdir。
- 2026-09-02：headless Chrome 里 localStorage 在 about:blank 上下文不可访问；跨导航持久的 localStorage 属于 profile 而非页面，批量走查用固定 profile 或单会话内切格。
- 2026-09-02：并行协调会话共享同一仓库 refs，本分支的提交可能在收尾前已被先行合入 main——rebase 时空提交去重是正常现象，收尾必须核对树里实际有什么（git show --stat 加关键文件 grep），再决定文档怎么写。
- 2026-09-03 W6：多波会话共享同一仓库时，协调者的 ff 合并目标随时移动（本轮连续两次 ff 失败：S1 沉淀分支与 S3 分支都被其他会话推进的 main 甩下）——合并前必须重新 fetch 核对 tip，ff 失败的唯一动作是临时 detached worktree 里 rebase 后 force-with-lease 重推，禁止 merge bubble。
- 2026-09-03 W6：派单会话的分支可能被其他波次会话顺手吞并进 main（S3 四个提交未经协调者之手就入库）——收合并前先跑 git log main..branch 判空，为空就只剩清点与补漏，不要重复合并。
- 2026-09-03 W6：小型机械修复（dialog 家族补 forwardRef，含测试与双门禁）协调者亲做全程约 15 分钟，比再开专属会话的往返快得多——派单粒度下限：预计 30 分钟内且路径明确的修复不派单。
- 2026-09-03：同一组件在 A 页组合能渲染不代表 B 页组合能跑——context 消费（useFormContext 等）的 provider 覆盖是按挂载点独立的；把组件复用到新页面先问「它的 context 谁提供」，组件测试按每个真实组合挂载（不带外部 provider 的组合尤其要测）。
- 2026-09-03：react-hook-form 的 useFormContext() 类型签名声称非空，运行时在 Provider 外返回 null——TS 对 context 缺失零防御，解构崩溃只在运行时爆；凡 context hook 返回值都按「可能 null」对待。
- 2026-09-03：测试红→绿要双向证明——写完回归测试先在未修复代码上跑一遍确认必红，再应用修复确认转绿；单向「绿了」无法区分「测住了」和「本来就绿」。
- 2026-09-03：连接/会话类测试只断言「第一个成功事件」是系统性盲区——dialhop 系用例全部止步于 PeerConnected，没有任何用例断言「此后观察窗内无 PeerDisconnected」或「连接仍可承载真实往返」，闪断类回归必然漏网。连接测试的最小完备断言：成功事件 + 观察窗零断开事件 + 一轮真实 request/response。
- 2026-09-03：跨层事件语义接线（发现层 Expired → 连接层 Disconnected 这类一行映射）要有自己的测试——它改变用户可见语义，却既不在源层单测也不在目标层单测的覆盖内。
- 2026-09-03：诊断测试的 helper 缺陷会把根因带偏（echo handler 只注册单侧，双向 echo 失败被误读成连接分家的直接证据）——诊断结论落地为回归测试前，先单独验证 helper 自身（两侧各测一次已知通路）。
- 2026-09-03：消融证明要逐点对齐——每个回归测试对应它实际触发的那条映射路径（E7-K2 第一轮撤了握手错误路径却没撤 connect_with 路径，对应测试仍绿=假消融）；撤错点不算消融，测试各自对应的点全撤完、全红才算。
- 2026-09-03：cargo test 默认 fail-fast，第一个失败 target 即停——消融/红绿记录一律加 --no-fail-fast 才能拿到完整红绿矩阵。
- 2026-09-03：cargo fmt 改写文件后 edit 工具的 read 缓存失效报 "file changed since it was read"——fmt 之后必须 re-read 才能 edit。
- 2026-09-03：错误链保真的机械口径：io::Error 载荷必须经包装器补 source()（std 盲视陷阱）；冻结错误契约只加变体不改形状——Dial/Handshake 文本变体保留给「无内层错误对象的契约性拒绝」，新增 *Chained 变体承载 #[source] 链。
- 2026-09-03：给冻结枚举加事件变体前先 grep 下游是否穷尽 match（swarm 对 RelayEvent 三变体+None 全臂匹配，加变体即破坏并行会话的 crate）——穷尽匹配在场时「复用既有变体 + 归因化 reason 字符串 + WARN 日志」是零契约破坏的上抛通道。
- 2026-09-03：集成测试里「控制流关闭清理」与「在途 connect 处理」跨流竞态不可赌调度顺序——用服务端可观测水位（metrics 快照轮询到目标值）做确定性同步点，或断言「两种落地都正确的并集」；靠 sleep 排序是假确定性。
- 2026-09-03：客户端派生的常驻任务（保活/读循环）必须在 Drop 里 abort——任务持有的流写半 Arc 会延迟对端 EOF，静默破坏既有回收语义（churn 回归 40 轮自锁正是这么被引爆的）。
- 2026-09-03：300 行红线预算要按 rustfmt 之后的行数算——chain_width=60 下 .await.expect() 链超 60 字符必被拆行，280 行手写稿 fmt 后变 350；先 fmt 再数行再提交。
- 2026-09-03 E6：跨 crate 共享的 config struct 若被上游用「全字段字面量」构造，该形状即事实冻结——加任何字段都逼只读方同步改；加参数先 grep 全部构造点，走新增装配入口/builder（SwarmConfig 之于 facade 实证）。
- 2026-09-03 E6：共享事件枚举「只增不改」加法变体仍会打断白名单式消费方（recv 一个事件并 match 特定变体的用例）；给新事件家族选独立通道（等价事件机制）或先 grep 全部 recv/match 严格度，别信「加法=兼容」。
- 2026-09-03 E6：集成点设计前先看目标文件行数余量（mod.rs 恰好 300 行时任何接线都放不下）——先做零行为搬移（内聚块独立成文件）腾余量再接线，红线检查脚本提前跑一次。
- 2026-09-03 E6：drop(Arc<T>) 不等于关停——accept 循环等后台任务持强引用时测试里的 drop 是 no-op，连接根本没拆；模拟对端死亡必须显式调 shutdown 类方法。
- 2026-09-03 E6：状态机测试的 setup 要按合法路径驱动到目标态（machine_at helper），不能单步 transition——Disconnected 到 BackingOff 这类 setup 本身就是被测的非法转移。
- 2026-09-03 E7：多兄弟分支逐支 ff-only 合入时，每合入一支 main 就前进，其余分支立即重新分叉——必须「rebase→合一支→再 rebase→再合一支」循环，不能一次 rebase 完批量合。
- 2026-09-03 GUI 邻居表复盘：展示层推断字段（来源=有 dial_hop 即手动、dial_hop 也刷新 lastSeenMs）会把「拨过」伪装成「在线」。权威字段该由底座透传时就透传（地址簿本就持有来源），推断式展示字段在契约里显式登记为缺口并限期补上，不要让补丁语义长期存活。
- 2026-09-03 E9-Q0：协调者在主树的未提交编辑是全工作区共享风险——编辑完必须立即提交，然后才能创建/唤醒任何会话。本轮 greeting 发出 83 秒后，coordination.md 未提交编辑即被新会话扫走提交（37c326a 实录，内容侥幸零改动）；顺序应为 编辑→提交→建会话。
- 2026-09-03 E9-Q0：勘误 21 号技巧——无参绑定工具 session_link_list 在当前宿主版本显式传 {} 可正常返回（无参直调仍报 lossless JSON）；升级后先试 {} 再绕路。
- 2026-09-03 E9-Q0：宿主重启后部分会话转 idle 而非死亡，几十秒内会陆续自行唤醒——协调者快照里 running=false 不等于会话丢失，重复派单前必须二次核对 updatedAt 与 worktree 文件活动，否则双会话同 worktree 相撞（本轮 23:10:46 快照误判、23:18 撤单止损，幸而新会话尚未落盘）。
- 2026-09-03 E8-M2：cargo fmt 必须在每个编辑批次内、commit 之前跑——提交后再 fmt 产生格式漂移，只能 fixup rebase 归位到正确提交（autosquash 三步：reset --soft 拆提交→--fixup→GIT_SEQUENCE_EDITOR=: rebase -i --autosquash）。
- 2026-09-03 E8-M2：协调者活跃提交期 ff-only 合并是竞态——把「worktree rebase main → push --force-with-lease → 主树核对分支后 ff-only」压进单条 bash 原子执行并循环重试；本轮被 docs 提交连续抢跑三次，第四次窗口合并成功，全程未删 worktree。
- 2026-09-03 E8-M2：流式后台任务的 job_output 首读即消费后续为空——长验收结论写进自管日志文件（重定向 + echo EXIT=$?），完成后 grep 文件取判决，不依赖任务流回放。
- 2026-09-03 E8-M2：编辑工具的「先读后改」按精确路径记账——主树读过的文件不等于 worktree 同名文件已读，worktree 内批量编辑前先按 worktree 路径逐一 read，否则中途 ToolCallError 打断批次。
- 2026-09-03 E9-T2/T4：DSH run_code 里大段文本用 JS 模板字面量会炸转换层（"Expected ',' got ';'" / "content is not defined" 两连炸），落盘内容一律改走 bash 引号 heredoc（<<'EOF'）或工具直写，模板字面量只留短字符串。
- 2026-09-03 E9-T2：自写扫描脚本零输出时先疑过滤器再信数据——fnlen.py 扩展名集合存了 'rs' 却拿 os.path.splitext 的 '.rs' 去比，整轮静默空结果；零命中结果必须人工抽一个已知样本回验脚本。
- 2026-09-03 E9-T4：收尾链式命令里 git 输出接管道（git merge | tail）会吞退出码，ff 失败后链继续跑、误删了未合并分支的 worktree；收尾脚本要么 set -o pipefail，要么 git 关键步骤输出不接管道直接打印。补救路径：分支引用仍在，git worktree add 回该分支 → rebase main → 重推重合，全程零丢失。
- 2026-09-03 E9-T4：TS 顶层 await 需同时满足 tsconfig module=ESNext 与 vite build.target>=es2022（默认 target 会报 TLA 不支持），动态 import 出 prod bundle 验收用 grep dist/assets 找导出符号名，比看构建日志可靠。
- 2026-09-03 落档轮：单人短命分支任务中途 main 被并行推进时，反向同步优先 rebase main + push --force-with-lease 分支再 ff-only——分支仅本会话消费，force-push 无旁观者，进 main 不留 merge bubble（docs/walkthrough-findings 实录：按 AGENTS.md 字面走 merge main，main 上多出 3b09e87 合并泡，核验方点名线性史偏好；与 118 号技巧的「rebase→force-with-lease→ff-only 原子化」同路数）。
- 2026-09-04 方案落档轮：文本要过 JS 模板字面量→bash 单引号→perl/sed 正则三层时转义层数必错（写四层反斜杠、perl 静默零替换）；含 markdown 反引号的大段文本落盘后统一用 edit 工具 replace_all 清理单一短序列（如反斜杠+反引号两字符），绕开全部转义层。
- 2026-09-04 同轮：bash 单引号闭合后与路径粘连（漏空格）时 grep 静默退化为读 stdin，返回 0 匹配制造「已清理」假象；grep -c 的结论必须先用 grep 工具（非 shell 拼接）复核目标文件，再信「替换成功」。
- 2026-09-04 负载选路轮：cargo 不在默认 PATH 时管道接 tail 会双重吞错——输出 "cargo: command not found" 但 exit 0（退出码取自 tail）；bash 里凡 cargo 必先 export PATH="$HOME/.cargo/bin:$PATH"，判退出码必 set -o pipefail，管道后 echo EXIT=$? 才可信。
- 2026-09-04 负载选路轮：长任务期间并行会话两次推进 main（一次在实现中途、一次恰在 ff-only 与清理之间），反向同步不是一次性动作——ff-only 前必须重查 main tip，分支可能需要二次同步 main 并重跑门禁；125 号的 rebase→force-with-lease 路数可少留合并泡（本次按 AGENTS 字面走 merge，main 上多出两个 merge node，下次改用 rebase 路）。
- 2026-09-04 负载选路轮：并行会话的重构可能与你的新文件语义重复（对方先合了 degrade.rs 阶段化拆分，我另有 degrade_hop.rs）——解冲突时以对方文件为底、只重放自己的小增量（健康槽接线、事件 detail）并删除重复文件，比两条实现各让半步的折中合并冲突面小一个量级。
- 2026-09-04 负载选路轮：整数 EMA 权重 1/4 自零起步时亚毫秒采样永远收敛到 0（(0*3+1)/4=0），测试当场抓住——高频小量观测要在内部用高精度单位记账（微秒），只在 API 边界向上取整换算显示单位（毫秒）。
- 2026-09-04 会话清理轮：管理类 list 输出（workspace_list/session_link_list）≠ 用户界面真相——返回全量含已归档与残留行，验收清理效果必须看过滤面（archivedSessionIds、磁盘目录是否迁走），否则会误判「清理没生效」。
- 2026-09-04 远程支持方案轮：技术拓扑选型先问「价值住在哪里」——能力放客户侧等于把核心资产交付给用户（客户装上 agent 就不再需要你），商业公理可以否决工程最优解；被否决的拓扑与理由要写进设计文档存档，防止后续轮次重新发明。
- 2026-09-04 远程支持方案轮：大段 markdown 落盘的零转义路——内容按行组装成 JS 单引号字符串数组（先断言全文无 ASCII 撇号），拼 bash `cat <<'DOCEOF'` 引号定界 heredoc，反引号/美元/反斜杠全部字面通过，比 126 号的多层转义再清理少一轮。
- 2026-09-04 GUI 节点资料轮：`cargo test 2>&1 | tail -N` 会用 tail 的退出码掩盖 cargo 失败报 exit 0 假绿——判定一律用 `cmd > log 2>&1; echo EXIT=$?` 先落码再 grep，或管道前 set -o pipefail；「命令链 exit 0」必须与日志尾部交叉核对。
- 2026-09-04 GUI 节点资料轮：并行主树上长前台命令会间歇性静默无输出——长构建/测试转 run_in_background 后台 job，job_output(wait) 收结果，比前台硬等可靠且不阻塞并行步骤。
- 2026-09-04 GUI 节点资料轮：cargo test 全红时先做所有权取证（git diff 上游点..HEAD --stat <失败目录>）再动手——本轮 make check 连挂两次（fmt-check/test/doctest）全是并行会话刚落的 crates 改动与编译管线瞬态，与自己的 GUI 改动零交集，机械代修（fmt）+ 复跑定性（瞬态）即可，不抢别人 scope。
- 2026-09-04 RS P0b 轮：跨任务接缝需要装配级测试兜底——E2E 夹具自建 Enforcement 绕过了 main.rs 生产装配，ShellWhitelist::empty() 集成缺口三层验收（单测/E2E/门禁）全部绿灯通过，直到人工盘点才暴露；装配路径必须有自己的非空/连通断言。
- 2026-09-04 RS P0b 轮：完工报告里的「自曝遗留」必须在验收时逐条判定归属并回写账本——「白名单数据仍为空闭集」一行小字即生产全拒缺口；验收判据=exit code + 需求逐条对表 + 自曝遗留逐条处置，三者缺一。
- 2026-09-04 T36 轮：协调方指定专属 worktree 路径时先确认路径存在/获准再动手——任务书未写路径不等于可自选，本次自选 /Users/imeepos/ext512/p2p-t36 被强制迁移到 .worktrees/t36-chat-boundaries，来回多花一轮；同分支不能同时 check out 在两个 worktree，迁移前先 remove 旧 worktree 再 add 新路径。
- 2026-09-04 T36 轮：cargo 冷编译任务塞给 subagent/dispatch_task 会撞 600s 墙钟上限（两次 10 分钟白烧）——重构建/长测试一律自己用 bash run_in_background 起 job，job_output(wait) 收集，别委派给会话型工具。
- 2026-09-04 T36 轮：后台 bash job 的权威退出码在 job_list 的 detail 字段（"exit code: 0"），输出重定向/日志缺 stdout 时以 detail 为准，不要靠翻日志猜。
- 2026-09-04 T36 轮：bash 工具偶发 spawn ENOENT（code worker 瞬断）——隔 1-2 分钟自动恢复，用 job_list 等轻调用探活，勿连续重击。
- 2026-09-04 T36 轮：重派任务书修正验收 cd 深度时先复算再执行——apps/gui/src-tauri 的 ../../.. 恰为各 worktree 根，p2p-t36 与 .worktrees/t36 两处语义一致，无需改命令只需换 workdir。
- 2026-09-04 IM 好友流轮：测试从 fixtures 出发而非从用户旅程出发——fixtures 让数据从天而降，「第一个数据从哪来」永远没被测到；后端/IPC/mock 三层各自全绿看不见「GUI 零调用点」断链，只有从零开始走用户路径才暴露（用户开箱卡死第一步后才被发现）。边界测试越测越细不等于覆盖：细度堆在已建成模块上，方向偏了越勤越糟。

- 2026-09-03 T36 边界轮：rustfmt 默认 small-heuristics（struct_lit_width=18）会把一行宽结构体字面量展开成多行，写测试文件时按「fmt 输出形态」设计：预绑定中间变量、用元组/构造函数代替宽结构体字面量、json! 宏内容不被 fmt 重排可放心单行。
- 2026-09-03 T36 边界轮：run_code 用 JS 模板字符串搬运 Rust 源码会被内容截断，大文件一律走 bash 单引号 heredoc（cat <<'EOF'）。
- 2026-09-03 T36 边轮：edit 工具的 old_string 必须匹配 rustfmt 之后的磁盘真实文本，凭上一次记忆改必失配；每次 bash 写盘后要重新 read 再 edit。
- 2026-09-03 T36 边轮：GUI build_node 对空 bootstrap/relay/observation 会回退出厂云端端点（config.rs default_*），离线测试配置必须显式传回环占位地址，否则测试悄悄连公网。
- 2026-09-03 T36 边轮：chat_send 对「已登记但死亡地址」的拨号降级链（直连→打洞→中继）耗时 >30s，命令层投递验收要用双回环真实节点拿 delivered=true，不要赌死地址快速失败。
- 2026-09-03 T36 边轮：并行边界轮多会话同仓作业，worktree 会被协调会话清理、main 随时前进；开工前 git worktree list + 分支重置到最新 main，交付后立即 commit+push 不留未提交状态。
- 2026-09-04 IM-T44 轮：itest 夹具监听端口写死（哪怕只是测试用）就是潜伏 flake——其他进程占住端口即 AddrInUse 假红、空闲时全绿，归因极难；夹具一律 quic_port(0)/tcp_port(0) 内核分配，重启场景（同 data_dir 同身份）端口必变后用 friend_add upsert 把新 listen_addrs 刷新进对端地址簿（与真实应用重连发现同语义），facade 默认配置本就是端口 0，别画蛇添足传固定值。
- 2026-09-04 IM-T44 轮：修复「占端口假红」的消融两步要同轮留证——修复前占住原固定端口复现红（panic 栈指到夹具 build 行），修复后不释放占端口进程跑全量全绿（lsof 实证占用仍在），才能证明修的正是端口依赖；只报三次连跑绿，无法排除「本来就绿」。
- 2026-09-04 IM-T41 轮：并行轮 main 每 1-2 分钟落一个 docs 提交，rebase 后先跑 4 分钟门禁再 ff-only 必连吃三次 Diverging——先 git diff --name-only <旧基线> main 确认增量纯 docs（与我的代码零交集、且同内容树刚全绿过）则跳过重跑门禁，rebase→push→ff-only 压成一个原子序列立刻执行，一次过；ff-only 失败严禁删 worktree，回 worktree rebase 重试即可。
- 2026-09-04 IM-T41 轮：测试夹具 PeerId 别用「字母表轮转拼 44 字符」的假 base58（旧 chat-view.test 同款生成器）——随机 base58 串解码常是 33 字节，前端按后端同口径校验（bs58 解码恰 32 字节）会正确拒绝它，夹具必须用真实 32 字节的 base58 编码（node BigInt 移位编码 30 秒可产出）。
- 2026-09-04 DSH 轮：同一仓库里概念相同、插件不同的配置键名会各自为政（llm-deepseek 用 `inputModalities`，llm-pi-ai 用 `input`），schema 对未知键静默透传——配置「写了没生效」先怀疑键名抄错了插件，对照目标插件 config.ts 的 schema 字段，别凭另一个插件的写法类推。
- 2026-09-04 DSH 轮：验证运行中服务的配置热加载，不必碰服务进程——用服务同款解析代码在 Node 里直跑配置文件（Node 24 原生跑 .ts），打印解析产物即为最终事实；先确认进程实际读哪个文件（profile 参数只换插件组合不换 home，settings 固定在 `$DSH_HOME/settings.yaml`）再动手改。
- 2026-09-04 IM-T42 轮：GUI 的 IPC 调用点守卫是「views/components 非测试文件里出现方法名字面量」的机械 grep——把 ipc.chatXxx 藏进 stores/ 层守卫照样红（静态守卫测试与验收命令双杀）；既定模式=对话框/组件直调 ipc.chatXxx，store 只做本地状态收尾（T41 chatFriendAdd 直调先例，T42 先写进 store 被红后对齐）。
- 2026-09-04 IM-T42 轮：DSH 后台 job 的 job_output 是流式一次性消费，wait 读过后二次读返回空、结论就丢了——验收类长命令一律重定向到 /tmp 文件留证，判读以文件为准，别指望回读 job 输出。
- 2026-09-04 IM-T42 轮：`cmd | tail` 后 `$?` 取的是管道尾（tail）的退出码，测试红了 exit code 也显示 0——验收成败判定要么 `cmd > log; echo $?` 紧跟命令，要么以落盘日志里的 FAIL/OK 标记为准。
- 2026-09-03 CL2 轮：客户端包装函数成对出现（call/call_slow…）时行为极易漂移——call_slow 复制连接代码却漏掉响应拆包，出现「守护进程响应正常、客户端解析失败」假象；公共路径单点化（raw_call + unwrap_response），包装只差超时参数。
- 2026-09-03 CL2 轮：文件行数预算前置设计——机制（lifecycle.rs）与命令面（node.rs）一开始就分文件，比写完 350 行再拆便宜；事实源结构（Report）放机制侧，命令面只留子命令分派与文本渲染。
- 2026-09-03 CL2 轮：CLI 等价 GUI 的长驻能力用「pidfile + UDS JSON 行协议 + log 落盘」三件套即可，不必上 RPC 框架；控制请求统一 {op,...} 信封、响应统一 {ok,data|error}，新操作只加 op 分派一支。

- 2026-09-04 T35 检查轮：目录/分支名与任务同名不代表工作发生在那里——协调者笔记「现场全清」仍漏壳（p2p-t35-gui 的 test/t35-gui-chat-boundaries 从创建起零提交）。归属判定三步：`git reflog show <分支>`（只有 Created from 一条即空壳）+ `git rev-list --count main..<分支>`（0=无独有提交可安全清理）+ 任务真身以主树账本 mergedMain 哈希为准（T35 实际走 test/im-bt35-new，e6b98ea 合入）。
- 2026-09-04 T36 检查轮：卡 done 合入 ≠ worktree 收口——同一 worktree 可能在验收之后继续长出未记账的增强提交（11fe0a6 晚于验收合入 1h13m，账本零认领）。处置旧 worktree 前四查：账本认领（grep 提交哈希/分支名）+ `rev-list --count main..<分支>` 独有提交 + `git ls-remote` 远端备份 + main 是否动过同文件（判合并风险）。零独有提交的壳才可删；有实质未合并工作的只报告不擅动，收口归属其执行会话/协调者。

- 2026-09-03 CL3 轮：并行波会改公共签名（本轮 Chat::send 被 T36 增第 5 参 reply_to）——合并 main 后必须全量 cargo build + cargo test 再提交；E2E 脚本里「二进制存在就跳过构建」的惰性构建会用陈旧二进制假绿，验收前强制重建一次。
- 2026-09-03 CL3 轮：底座 chat 链路（含 itest）按 TCP 地址验证——CLI E2E 建好友取 listenAddrs 里 /t 地址，取 /u（QUIC）会间歇性第二发消息 failed（连接重建竞态）。
- 2026-09-04 IM-T46A 轮：并行会话高峰期，门禁绿与 ff 合并之间别做任何别的事——本轮验收绿后 main 被推进两次，每次都得 rebase+全量复跑（约 4 分钟/轮）；零交集 rebase 很便宜，但门禁必须在新基线重跑，合并动作本身要抢窗口立即执行。
- 2026-09-04 IM-T46A 轮：`git diff --name-only <branch>..main` 列出的是双方全部差异（含自己分支的改动），grep 它当「文件交集检查」会满屏自己——交集判断要看对方新提交碰了哪些已知范围。
- 2026-09-04 IM-V1 轮：并行会话高峰期主树 make check 会瞬态红（target 目录锁/缓存竞争，本轮首次 exit 2 零源码错误）——主树验收失败先原样重跑一次再定性，增量编译自愈即假红；连续两红才立案。
- 2026-09-04 IM-V1 轮：识图任务派给无视觉输入的模型（如 GLM-5.3-Flash）时 read_image 直接拒载——自查手段降级为 headless Chrome --dump-dom 断言关键类名/DOM 标记 + 截图入库存档留人工复核，不要在 inability 上反复重试。
- 2026-09-04 IM-V1 轮：范围受限单（如「只改 views/shared」）需要共享布局效果时，用页面级 Tailwind 任意变体（[&_[data-slot=card]]:h-full）替代改共享组件——不越 scope 拿到同等效果，shared 只留真正跨页复用的规范组件。
- 2026-09-04 GC1 轮：gif 0.13 `Frame::from_rgba_speed` 无 Result 直返 Frame（没有 map_err 可挂）；`gif::Encoder` 借用输出缓冲直到 drop——函数返回缓冲前要显式 `drop(encoder)`，否则 E0505。
- 2026-09-04 GC1 轮：并行会话同仓库时 `push origin main` 报 up-to-date 不是事故——其他会话可能刚推进了 main；用 `git rev-parse main origin/main` 确认自己的合并 hash 已在远端历史（是祖先）即可，别重推别慌。
- 2026-09-04 GC1 轮：headless cargo test 构造不了真实 WKWebView（macOS 事件循环必须在主线程）——GUI 渲染管线用「帧源 trait + 合成帧源」注入集成测试：HTTP/鉴权/编码/校验/落盘全链路真跑，只有 OS 抓帧一层留给运行态验证。
- 2026-09-04 IM-T45 轮：react-refresh/only-export-components 禁止组件文件同时导出 hook——hook 放 store 模块或独立 .ts（非组件文件不触发该规则）；条件调用 hook（a ? useX() : b）同样是红线，纯展示组件让调用方注入状态值。
- 2026-09-04 IM-T45 轮：测试辅助函数对 store 做增量归约时，reduce 基准必须是当前状态（setState((s)=>…reduceEvent(s,…))），写死空状态会把先前事件静默清空——「第二次 apply 丢第一次」类断言翻车先查基准。
- 2026-09-04 IM-T45 轮：新需求（去重主操作）与既有测试契约（双入口同时可见可点）冲突时测试契约优先——做视觉层级分化（空态中央按钮升 default 变体）而非物理移除入口，commit 正文写明取舍依据。
- 2026-09-04 IM-T46B 轮：react-refresh/only-export-components 对常量与纯函数同样生效（T45 只记了 hook 情形）——组件文件混导出 summary 工具即双 error，拆独立 .ts 模块（reply-summary.ts 先例）。
- 2026-09-04 IM-T46B 轮：apps/gui 的 hardcoded-copy 守卫只剥整行注释，行尾 `// 中文` 必被 CJK 扫描拦下——注释一律写在代码上方独立行，新增文件首跑一次该测试再进 build。
- 2026-09-04 IM-T46B 轮：eslint react-hooks/set-state-in-effect 禁止 effect 内同步 setState——「props/选择变化要清子组件状态」走事件路径（onSelect 回调里重置）而非 useEffect；跨会话不匹配的旧高亮 id 留着无害，不为清而清。
- 2026-09-04 IM-T46B 轮：i18n 严格键类型（CustomTypeOptions）下 t() 的 key map 必须 as const、返回 (typeof MAP)[ChatKind]，宽化成 string 直接 tsc build 红；vitest 不做类型检查所以测试全绿也拦不住，build 是唯一闸口。

- 2026-09-04 IM-V2 轮：无视觉输入的视觉任务交付物 = 类名/DOM 断言测试 + CDP 几何实测
（bounding box/computed style）+ 前后截图留档三件套；协调者识图复核只认这三样，
"门禁绿"与"视觉生效"是两条独立判定轴。
- 2026-09-04 IM-V2 轮：无视觉模型先试一次 read_image 探明能力（GLM-5.3-Flash
  直接拒绝 image input），确认后立即转 CDP 路线，别在识图上浪费轮次。
- 2026-09-04 GC2 轮：二进制 magic 常量别手算十六进制——`echo -n GIF | xxd -p` 现场生成对照（本次 GIF 写成 1f4946，正确 474946，真机产物正常生成仍被判非法，白跑一轮验收）。
- 2026-09-04 GC2 轮：macOS TCC 对 ad-hoc 签名二进制按 cdhash 记授权——重建即失效；字节相同的副本（桌面 .app 外壳包同一可执行）与原件同 cdhash，授权互通；权限预检每次实时查 TCC，授权后连存量进程都能直接过预检，但旧代码进程的缺陷仍在。
- 2026-09-04 GC2 轮：开工缺陷修复前先 `git worktree list` + `git branch -a` 查并行会话是否已占同主题分支（本次撞见 fix/gc-capture-callback 空分支同修一个缺陷）；撞上先只读对方 diff 判进度，未提交则按协调者对本会话的明确授权继续，并在回报中标记撞车让协调者收编。
- 2026-09-04 GC2b 轮：接修复单先 `git fetch origin` 并 `git log origin/main --oneline -- <涉事路径>` 查是否已有平行修复，再动手写码——本轮双派单，独立完成同根因修复后才在合并冲突中发现 c76c41a/8c16ef7 已落地，两份实现只能弃一份；先查可省整轮实现+冲突消化。
- 2026-09-04 GC2b 轮：tauri `with_webview` 闭包在主线程执行——闭包内绝不能阻塞等「要投递回主线程的回调」（快照完成回调/file 协议回包同理），必自锁死；模式是闭包只发起 + channel 送回调用线程等待。
- 2026-09-04：AGENTS.md 写"远端名是 gitea 不是 origin"，但本仓库实测 git remote 只有 origin——仓库级惯例文件会过时或张冠李戴，涉及远端操作前先 git remote 实测再动手。
- 2026-09-04：git worktree add 不能检出已被其他 worktree 占用的分支（fatal: already used by worktree）；验证钩子/临时检出用 --detach，不占分支名。
- 2026-09-04：post-checkout 触发面：HEAD 级检出（分支切换/新 worktree/clone）都触发（flag=1 或 old=全零），git checkout -- <path> 路径级不触发（flag=0）——钩子内按 flag 过滤可避免路径检出误动作。
- 2026-09-08：edit 替换 import 块时 old_string 只锚了块尾两行，new_string 却按整块重写并重发了块内上文已有的行——立刻产生双 import（tsc 必红）。改 import 块前先确认 old_string 的覆盖边界，new_string 只写净新增行，改完立刻 read 回看。
- 2026-09-08：测试夹具给联合类型字段覆写字面量（state: "left"）先赋 const 再传 setState，字面量在声明处已放宽为 string——vitest 不查类型全绿，tsc build 才红（2026-09-04 键类型教训的同类：build 是唯一闸口）。沿用仓内 `as const` 先例或给夹具直接标目标类型。
- 2026-09-07 rail 轮：ff-merge 后并行会话会继续推进 main，收尾核验「我的提交是否还活着」用 git merge-base --is-ancestor <commit> main，别用 git log 头部比对——头部早就是别人的提交了（本轮 main 在我合并后 1 分钟内被 chat-paged-view 会话推进两个提交，ancestry 校验确认两个 fix 提交均在）。
- 2026-09-07 rail 轮：设计系统已有语义令牌（shadcn 的 --sidebar/--muted 系列）时要先找令牌再写死颜色——本轮侧栏「黑底不随主题」的根因就是 WX1 风格引入了固定色 --wx-rail，绕过了已有主题机制；删固定令牌 + 改语义类（bg-sidebar/text-muted-foreground/hover:bg-sidebar-accent）零 JS 改动即双主题自适应。
- 2026-09-04 N2：git stash pop 或外部脚本改写文件后，edit 工具必报 file changed
  since it was read——先重读再改，别凭记忆构造 old_string。
- 2026-09-04 N2：并行会话会在你验收窗口内推进 main（本次 ai-guide 会话把 main
  从我的合并点 ff+merge 到 214c41f）；ff 合并后尽快 push main，回报合并 hash
  用自己的合并点并注明 main 已前进到含它的后继提交。
- 2026-09-07 UX 终验：UI 验收断言抓「含目标文本的最小节点」会漏掉兄弟位置的按钮（撤回按钮与卡片标题是 siblings），判定以 bodyInnerText 全文证据为准；innerText 不含 placeholder/aria-label/title，复制 affordance 这类无文本交互必须单独查 attribute。
- 2026-09-07 UX 终验：场景断言失败先分「应用未修复」vs「断言口径过时」——修复本身会改 DOM 形态（字段标签变「地址 N」序号、二级选择页变页签、错误码变人话文案），拿修复后形态重写选择器后仍失败的项目才可定性为未修复；别把第一轮脚本当判决书。
- 2026-09-06 UX1 轮：zustand store 新增锁存/闸门字段（如 autoStartRequested）必须
  同步进所有测试夹具的 reset/prime 基线——夹具只重置旧字段时，上一用例消耗掉的
  闸门会跨用例残留，表现为下一用例「动作静默不触发」的假红；新增状态字段与夹具
  字段清单要同一 PR 内同步核对。
- 2026-09-06：接派单任务的第一道工序应是按协调方验收命令的原样 PATH 在基线上空跑一遍验收（本次暴露 `cargo test -p apps独立workspace包` 根目录解析失败、apps/cli 基线 test/clippy 红、bash 5.3 全角字符 bug 三处，全与本次改动无关）；收尾才发现基线红 = 被迫代修别人的域。
- 2026-09-06：cargo fmt 会格式化整个 crate（含 fmt 门禁未覆盖的独立 workspace 的存量漂移文件）——跑之前 git status 建基线快照，收尾 diff 超出自己文件域就是越界信号，漂移要么回退要么按提交纪律拆独立 style 提交。

- 2026-09-05 P0壳：设计文档内部有张力时（5.3 要求 /group /acp 重定向 vs 七、
  迁移策略要求群聊/ACP「P1 前保持整页形态挂新壳」），以可机械验收的清单为
  权威反推实现（重定向到 /chat?kind=*，kind 分支挂整页视图），两头约束同时
  满足；不要在文档两段间二选一，找同时满足两段的第三形态。



- 2026-09-05 PR1 轮：bash 后台启动 serve 的函数不能被 `VAR="$(fn)"` 命令替换
  包裹——子 shell 里 PIDS+=("$!") 只改副本，父 shell 数组恒空，pop 时报 bad
  array subscript；模式是函数内直接改父 shell 数组 + 就绪结果写全局变量带回，
  纯读函数才允许命令替换。
- 2026-09-05 PR1 轮：bash 生成脚本内容经 JS 模板字符串写入时，脚本里的
  `${...}` 会被 JS 当插值吃掉（parse error Expected ident）；改用逐行数组
  join 普通字符串承载，或全部转义 `${`。
- 2026-09-05 PR1 轮：本仓库 chat serve/一次性命令的身份锁要求数据目录已存在，
  mktemp -d 下的子目录直接当 --data-dir 会报「身份被占用…No such file or
  directory」（锁文件建不出来）；E2E 起节点前先 mkdir -p 各 data-dir。
- 2026-09-05 PR1 轮：长测试（workspace 级集成测试单件 20-90s）严禁前台同步
  等结果——600s 超时烧掉一轮；一律 run_in_background + 日志文件，期间做互不
  依赖的下游工作，只在真正被阻塞时 job_output wait。
- 2026-09-05 PR1 轮：同一个日志文件不能被两次后台运行复用（第一次残留与第二
  次输出交错假象）；每轮独立文件名或先清空。cargo 增量重链接全部集成测试
  二进制是分钟级操作，改 lib 后的验证优先 cargo test -p <crate> --lib 快筛。

- 2026-09-05 UB：run_code 里用模板串装含反引号/花括号密集的 shell 命令，两次在
  "解析 program" 阶段炸 Unterminated template（与目标文件无关）；长命令一律改成
  字符串数组 + join(" ") 拼参，模板串只留给无特殊字符的短串。
- 2026-09-05 UB：tools.edit 删文件尾部重复块时，old_string 若只含重复块本身会命中
  两处被拒；锚点必须带上目标块独有的相邻行（如前一个 describe 的收尾断言），
  先 read 全文核对匹配次数再动手。

- 2026-09-05：git commit 提交的是整个暂存区，不是刚 add 的路径——soft reset 重做
  提交序列时，`git add <path> && git commit` 会把 index 里所有遗留暂存卷进一个
  巨石提交。修法：要么 commit 用 pathspec 形式 `git commit -m msg -- <paths>`
  （按工作区状态只提交指定路径），要么提交前 git status 确认暂存区干净。
- 2026-09-05：对独立 cargo workspace 的子包（apps/cli 有自己的 [workspace]）跑
  `cargo fmt --manifest-path` 会把包内他域文件的存量格式漂移一并重写（根 fmt
  门禁只扫根 workspace，漂移因此长期潜伏）；会凭空造出他域文件 diff，撞并行 PR
  的冲突面。fmt 后必须 git status 核对，非本任务文件一律 checkout -- 回退。
- 2026-09-05：run_code 模板串里生成 JS/TS 代码时，行尾续行反斜杠与换行转义需要
  双重转义层级心算，很容易造出合法 TS 但非法目标语言的序列；写完立刻跑一次
  目标脚本冒烟（--help 级别即可），本次靠冒烟 30 秒内抓住语法错。
- 2026-09-05：RTL 的 getByText(函数谓词) 会命中多个元素（谓词跑在多个节点上），定位组件内文本用 data-testid 直读 textContent 最稳（UA 轮 use-hotkeys 探针四连败根因）。
- 2026-09-05：cmdk 在 jsdom 需要 ResizeObserver 与 Element.prototype.scrollIntoView 两个最小桩，缺一即面板挂载崩；桩放各测试文件头部，不动共享 setup.ts（他人所有文件）。
- 2026-09-05：行为断言写 toHaveBeenCalledWith(具体值) 而非 toHaveBeenCalled()，才能抓住类型系统放行的真缺陷——本次靠它抓到关闭回调吞参把 undefined 写回受控 open 状态。
- 2026-09-05 给跨域共享类型（如 EventStateSlice）加必填字段会编译炸掉并行 PR 域的测试文件（它们不可改）：新增字段做成可选+缺省回退（reduceEvent 内 (state.eventSeq ?? 0)+1），本域测试显式补全，契约语义写进注释。
- 2026-09-05 冷 worktree 跑 make check（含 cargo 全量冷编 + gui-tauri）>10 分钟，同步等待必撞工具超时：一开始就 run_in_background 落盘日志（make check > /tmp/x.log 2>&1; echo EXIT=$?），job_output 轮询。
- 2026-09-05 门禁跑批期间不改任何文件是硬规则：先把所有源码改完再起 make check；中途补丁会让该次门禁失去证明力，只能重跑全量。
- 2026-09-05 UC 会话事故复盘：对共享 append-only 文件跑 git restore 前，必须先
  git diff 全量核对——主树 lessons.md 里叠着他会话 14 行未提交追加，本次 restore
  连同抹掉，仅抢救出尾部三行（见下条恢复标记）。教训：只 restore 自己能逐行
  说清来源的 hunks；外来内容一律先另存再处置。
- 2026-09-05 恢复记录（自主树未提交残留抢救，原会话请以完整原文重录，本条仅防丢失）：
  「（Expected unicode escape）且整段程序不执行、其中所有已发工具调用全部回滚——
  大块代码文本搬移用「read 行切片 + write 整写」，长文本一律走 write 工具，
  不在 JS 串里手拼含转义的代码内容。」
- 2026-09-05 UD 会话：中央登记守卫测试（acp/group-registration）用正则
  /path="([^"]+)"/ 解析 App.tsx 源码——把 App.tsx 改成 createHashRouter
  的对象式 { path: "peers" } 会打碎它们（禁改文件不能适配）；正解是
  createRoutesFromChildren 保留 JSX 路由声明，data router 与登记守卫两全。
- 2026-09-05 UD 会话：eslint-plugin-react-hooks v7 有 refs 规则，render 期
  ref.current = x 直接报错（旧写法 hook 保最新闭包失效）；替代是 useEffect
  内同步 ref，或干脆 effect 内重注册回调（Map.set 幂等）。组件文件混导出
  函数触发 react-refresh/only-export-components——纯逻辑拆独立 .ts 文件。
- 2026-09-05 UD 会话：vitest 不做类型检查（测试文件跑得过），tsc -b 才拦
  vi.fn 签名/HTMLElement 属性错误——「测试全绿」后 build 仍可能红；先
  typecheck 再报绿。vi.mock 的模块工厂对象方法签名要和真实调用参数一致。
- 2026-09-05 UD 会话：useConfirm 这类 context hook，测试 harness 里把
  <Provider> 包在组件返回 JSX 内没用——必须包住调用 hook 的组件本身
  （render 外层），否则 useConfirm() 在挂载时即 throw。

- 2026-09-05：页面注册有三处并存清单（menu.def.ts / src-tauri control ROUTES / 前端 PAGE_REGISTRY），加页只改一处会静默漏——group 页曾三缺二；加页前先 grep 旧路由名全仓找齐清单，并同步数量守卫测试（page-registry.test 显式清单）。
- 2026-09-05：协调链里的验证步骤同样禁止管道收尾（`git rebase main 2>&1 | tail -2; echo RC=$?` 的 RC 是 tail 的）：一律 `> 日志文件 2>&1; echo RC=$?` 再 tail 日志，本轮该坑以「merge 静默未生效、worktree 未删」形态三犯。
- 2026-09-05：session_link_talk 对已完成会话可能返回 replied=true 且 reply 为空串——交付判定以仓库实况（worktree 状态/分支 tip/远端同步）为准，不采信回执形态。
- 2026-09-05：GUI 走查脚本用 p2pctl gui navigate 时传路由名（dashboard 而非 /）；发布预检做 DOM 巡检可完全绕开截图权限缺陷，且比 ui-regression.sh 多覆盖 group/acp。


AGENTS.md 的「远端名是 gitea」不是普适事实：本机 p2p 仓库只有 origin（github）。收尾四步推送前先 git remote -v 核对实际远端名再执行（2026-09-05）。
- 2026-09-05：run_code 里 bash 命令写成 TS 模板字面量时，shell 的 ${var} 会被 JS 层先行插值直接抛 Unterminated template——长 shell 脚本一律「行数组 + join("\n")」拼串，变量用 $var 不带花括号，绝不让 ${ 出现在 code 字符串里。
- 2026-09-05：git mv 后对「新路径」文件 write/edit 前必须先用 read 工具读新路径，旧路径读过不算（工具按路径记账）；perl -pi 批量改过的文件再 edit 同样要先重读，否则报 file changed since read。
- 2026-09-05：并行会话在主树留 staged 半成品时 ff-only 合并三步走：git diff --name-only main <分支> 与 git status --porcelain 取交集 comm 校验为零重叠 + 核对 main==origin/main → 直接 merge --ff-only（git 不碰零重叠路径，外来 staged 态原样保留）→ 事后绝不跑全量测试（主树是别人工作区）。
- 2026-09-05：主树 index 有他人 staged 内容时提交自己的文件，用 git commit -m ... -- <pathspec> 只提交指定路径，普通 git commit 会把别人的 staged 删除一起打包。
- 2026-09-05：RTL 断言同名词在多卡片出现（如「中继」既是拨号链行名又是排障链接文案、「中继会话」既是指标卡标签又是趋势系列名）时全局 getByText 必撞多重匹配——用 getAllByText(label)[0].closest("[data-slot=card]") 再取卡内 card-title 的结构定位断值。
- 2026-09-05：job_output 的 wait 有运行时 600s 墙钟上限，先超时的是等待不是任务——make check 级长任务 run_in_background 后用非阻塞 job_output 轮 job.status，tail 管道会在管道结束前不出文本属正常。
- 2026-09-05：DSH 的 devloop_scan 绑定调用报 binding arguments must be lossless JSON（harness 序列化缺陷）时，改用 bash 跑同等只读命令替代，不要反复重试绑定。
- 2026-09-05：规格「迁移 X 配置面板」而 X 实为别的语义（permission-grading 是请求应答分级模型，不是配置面板）时，以章节正文描述的目标态 + 分期验收行为准（「权限档变更对后续会话生效」只可能指策略配置），「迁移」按词源照搬会做出验收不达标的残件；落地时在回报中显式列出待负责人复核的解释点。
- 2026-09-05：mock 后端夹具要贴真实契约形状（peer 必须合法 base58-32）而不是沿用 mock 自身的宽松值（mock-peer）：表单前置校验与后端同口径后，宽松夹具会被前端正确拦截，测试红因是夹具不是实现；mock 白名单用 configure({ peers: [真实 base58] }) 对齐。
- 2026-09-05：测试里从数组反查实体用「取末位元素」而不是「拿 length-1 当 id 查」——id 与数组下标是两套序列（mock 权限帧 id 从 100 起），混用必假红。
- 2026-09-06：跨平台测试断言禁止写死单一平台的路径/格式形态（p2p-cli 日志目录断言写死目录末段=app 名，macOS ~/Library/Logs/<app> 成立、linux XDG ~/.local/state/<app>/logs 末段是 logs，ubuntu CI 恒红）——应对照被测平台函数做 wiring 断言，形态断言下沉到该函数自己的跨平台用例。
- 2026-09-06：测试里 yield_now 大自旋等待定时器驱动的服务端事件是调度假设（多核热队列下全程不 park，50k 次纯 CPU 微秒级烧完，300ms 墙钟根本没流逝）——改小步 sleep 加截止时间判红的轮询，事件发生即刻过、不发生显式红，双平台语义一致。
- 2026-09-06：pipefail 脚本里 printf 管道喂带提前 exit 的 awk 有 EPIPE 竞态：awk exit 关读端，printf 没在 awk 退前写完缓冲就炸——macOS 恒赢（假稳）、CI 慢机可输；改 here-string 喂 awk 无此失效面。
- 2026-09-06：本地 make check 绿之后又改了东西再推送，必须重跑门禁（哪怕只加一个函数）——本次 wait_until 签名漂移被自己扩展的 CI fmt 门禁当场抓获，白烧一轮 12 分钟 CI。
- 2026-09-05 底座 peer 流下传卡：接任务卡先 `git log --oneline -- <条目涉及文件>` 对账再动手——登记/任务书会滞后于代码现状（c3b260f 已把 trait 加参与 swarm 喂流落地，实际只剩文档留档与应用迁移；不看历史会重复设计或误判契约未定）。
- 2026-09-05 底座 peer 流下传卡：新写 Rust 文件进长门禁前先 cargo fmt --check 预检（根与独立 [workspace] 子包各跑一次，acp-agent 不被根 fmt 覆盖）——漂移是小 style 提交，白跑一轮 make check 是十几分钟。
- 2026-09-05 底座 peer 流下传卡：后台 make check 的输出经 `| tail` 缓冲会让中途 job_output peek 全程拿不到进度；改 `make check > /tmp/x.log 2>&1; echo EXIT=$?` 落盘，中途可 tail 日志看阶段，退出码以落盘 echo 为准。
- 2026-09-05 发布链路卡：「无密码」不等于「空密码加密」——rsign 密钥空口令加密时，tauri 签名必须显式导出空串 PASSWORD 变量，非交互 shell 才不会去开 /dev/tty（Device not configured, os error 6）；文档口径差一个词就是一次四平台发布失败。
- 2026-09-05 发布链路卡：自检红绿矩阵里唯一期望 rc=0 的绿场景是 harness 自身的照妖镜——本次 eval "export $*" cmd 把命令词当 export 的 NAME 参数，八个红场景全是假阳性 rc=1，绿场景如实变红才暴露 harness bug；写自检先让它证明绿路径真能绿。
- 2026-09-05 UI 审计修复轮：ui-regression 的 start_gui 会复用「健康外部实例」但探针不区分构建版本——验证新外壳时若装机的旧壳 app 在跑，group/acp 重定向行假红、截图走的是别人家 TCC 授权；对准新产物验证前先确认 endpoint.json 指向的是刚构建的调试二进制。
- 2026-09-05 UI 审计修复轮：给既有 bash 大脚本加新「调用形态」前，先在目标机器 /bin/bash（常是 3.2）下复跑一遍最小入口；老脚本的历史绿只代表老调用路径。
- 2026-09-06 IMC1 卡：同一邀请在 owner/受邀者是两本独立台账，条目 id 各自生成互不相等——accept/reject 只能用本机 group_invites_list 里的 id，拿对端条目 id 必 NotFound；跨端关联靠 groupId+对端 PeerId，不靠 id。
- 2026-09-06 IMC1 卡：根 workspace members=crates/*，apps/cli 既不入 workspace 也不被 make check test（ai-docs-sync 只 build 不 test）——cli 测试腐坏无门禁拦（存量 FriendUpdateReport 测试缺 addrs 字段烂了很久），动 cli 域时手动 cargo test --manifest-path 补一次。
- 2026-09-06 IMC1 卡：验收链跑期间改源码=结果作废：cargo test 段跑的是改前二进制、fmt-check 在链尾才炸，白等 6 分钟；源码定稿后先 cargo fmt --check 再挂链，链跑期间冻结一切文件写入（含 docs/.md）。
- 2026-09-06：组件内聚 useNavigate 等路由钩子时，钩子组件必须「按需挂载」（条件渲染 null）——MessageList 常驻渲染导航弹框让 6 个裸渲染既有测试崩在 useNavigate() invariant；修复是把导航收进弹框组件并按需挂载，既有测试零改动回绿。
- 2026-09-06：run_code 跨调用无运行时内存再实证两次：上一调用定义的常量（如目标路径 p）在下一调用不存在（ReferenceError: p is not defined）；每个 program 必须自带全部常量与路径。
- 2026-09-06：read 全文→write 回写是大文件截断陷阱（792 行被 read 输出预算裁成 341 行后覆盖落盘）；追加用 bash cat >> heredoc，写后 wc -l 对账（IMC3 轮实录，合并后才被 diff 行数暴露，当场修复）。

- 2026-09-06：新 worktree 首跑 pnpm 脚本会触发 verify-deps 自动 install，长 install 撞前台超时被 SIGTERM（exit null 假失败）；建 worktree 后先单独 pnpm install 再跑任何脚本。
- 2026-09-06：长门禁链必须 run_in_background + 输出重定向日志 + 末尾 echo EXIT 到日志，判定看日志里的 EXIT 行而非 job status。
- 2026-09-06：run_code 里模板串写文件内容时，内容不能再含模板串或插值序列（外层被截断报 parse 错）；复杂内容先 write 到 /tmp 再 bash 拼接。
- 2026-09-06：并行波派单后 main 会持续前进（在途分支陆续并入消失），收尾前 fetch + merge main 在 feature 侧消化；overlap 检查对已删分支要容错。
- 2026-09-06 AS2 分享链接直拨轮：对 watch/broadcast 快照做「只认见过中间态」的门控会被通道合并语义击穿——拨号秒失败时 Connecting→Offline 合并成一次 changed()，中间态永远观察不到，真实失败被误判为陈旧快照挂到超时；多阶段迁移的归属判定要用时间锚点（迁移 since >= 本次尝试起点），不要用「是否目击过中间态」。
- 2026-09-06 AS2 分享链接直拨轮：`cargo test | tail` 这类管道会吞掉 cargo 的退出码（exit 0 假绿）；跑门禁必须 set -o pipefail 并显式 echo ${PIPESTATUS[0]}，验收口径里禁止裸管道收尾。
- 2026-09-06 AS2 分享链接直拨轮：run_code 里给 write/edit 传多行文本时，JS 模板串里的反引号必须转义，漏一个就是整段语法错白跑一轮；多行内容用「字符串数组 + join("\n")」组装最稳。
- 2026-09-06：同意制入群邀请两侧账本各自 id（A 的 out 条目 id ≠ B 的 in 条目 id），跨端测试按 groupId+direction 定位本端条目再操作，别拿对端返回的 id 当全局键。
- 2026-09-06：cargo fmt --manifest-path 指向独立 workspace 包（apps/cli）会把该包全部存量格式漂移一并重写（13 个非本卡文件被扫进 diff）；fmt 后 git status 对账，非本卡文件一律 git checkout -- 还原。
- 2026-09-06：apps/cli 包名是 p2pctl 且自带 [workspace]（根 workspace 成员只有 crates/*）：cargo test -p p2p-cli 跑的是 crates/p2p-cli 小库，p2pctl 单测门禁不覆盖，交付 CLI 改动后手动 cargo test --manifest-path apps/cli/Cargo.toml 验证。
- 2026-09-06：tauri mock-runtime 测试助手里 State 生命周期挂在 Manager 上，不能随函数返回值带出；照 group_command_smoke 模式返回 (App, AppHandle, 数据)，用例体内 handle.state() 自取。
- 2026-09-06：断言恒 false 的字段先读实现判断它改的是 store 还是内存副本；用钓鱼定位——在失败路径直调底层门面打印真实返回值，一次钉死黑盒差异（本次 1-8ms 即据此排除拨号超时假设）。
- 2026-09-06 IMC 轮：巡检他会话 worktree 一律走 git worktree list 实况，禁止按猜测路径探测（p2p-imc3 探空实为 p2p-imc3-gui，险些误判未开工）。
- 2026-09-06 IMC 轮：长任务会话「脏文件静止」不等于死亡——按 worktree 在位→文件写入→宿主进程（ps 找 cargo/make check）三级证据链依次定性；IMC1 验收长跑 50 分钟零写入但进程活跃。
- 2026-09-06 IMC 轮：共享账本/协调表编辑禁多层转义内联脚本（bash 套 python 套 JSON 必炸语法），用 write 落脚本文件再执行；改前先读回、改后回读校验（轮 44 教训复验）。
- 2026-09-06 IMC 轮：自己验收作业运行期间，主树禁止任何写操作（含 ff 同步远端）；编辑共享文档前必须重新 read（并行波次随时插提交，本轮 coordination.md 两次变化）。
- 2026-09-06 IMC 轮：磁盘余量是验收前置检查项——/tmp 历史波次 acc target 残留 66Gi 致 100% 满，验收作业只进 stderr 静默失败；清理前先 ps 确认无进程引用，在途波次 target 保留。

- 2026-09-06 后台门禁用 `make check 2>&1 | tail -N` 时管道吞退出码（job 报 exit 0 实为 tail 的 0），必须 set -o pipefail 并 echo ${PIPESTATUS[0]} 取真值。
- 2026-09-06 workspace_session_manage archiveSession 返回的 archivedSessionIds 是累计归档登记不是本次增量，单会话归档后核对以 session_link_list 可见性为准，勿被吓到误判批量误归档。
- 2026-09-06 .devloop/loop-state.json 用 python json.load+dump 整文件重写会把全文件重排序列化（diff 652 行噪音）；小改动优先 edit 工具做定点替换，整文件重写仅在结构性变更时用。
- 2026-09-06 ff 合并前 merge-base --is-ancestor 检查必做：主树被并行线持续推进时「刚验完的分支」随时可能不再含最新 main，链条断在 is-ancestor 就回 worktree 再 merge 一次。

- 2026-09-06 UX 波：并行会话对共享经验文件（本 references/*）做全量覆写=丢更新事故——本日 lessons.md -131 行/techniques.md -83 行实证，且多条目被字面 \n 折叠成单行。铁律：喂回一律 python 读文件→尾部 append→写回，写后 wc -l 必须单调递增；禁止凭记忆全量重写。
- 2026-09-06：crate 内把类型拆到新模块后，原模块的 `use` 再导出默认私有，跨 crate 依赖方（apps/cli）E0603 崩；本 crate clippy/test 全绿完全无信号。修复是拆分后立即 grep 全仓引用点并跑「验收命令全文」（含下游 crate clippy），只跑自身 crate 门禁等于没跑。
- 2026-09-06：程序化按行号切片搬代码（splice 三段）必留残渣：漏导入（serde_json::Value）、漏改调用点限定名、doc 注释 `//` 与 `//!` 混用全冒出来了。修复是搬完先 cargo check 全量读回两个文件再继续，不要信切片边界。
- 2026-09-06 契约无某字段不等于该能力做不了：先找系统内已有的权威数据面（实例：§15 status 无 peer，就从 console 发现面解析），并把解读口径显式报给协调会话留档，而不是私自改契约。
- 2026-09-06 mock 文件有膨胀趋势时要随 feature 拆独立文件（mock-ipc 已 400+ 行，新 mock 面 monolith 化会让行数红线与合并冲突双输）。
- 2026-09-06 表单收敛类改造要先把既有测试当「不可改约束」读一遍再动手：保留原 testid 与可空语义（如 peer 分享场景可空），新语义（必选校验）挂在新交互入口上，而不是改公共校验函数。
- 2026-09-06 组件级测试断言别断言 store 内部（draft.peer），断言用户可见面（触发器文案/渲染节点）——store 是实现细节，form 是组件局部态。
- 2026-09-06 UX 波协调：run_code 里 bash stdout 超 ~30KB 会被截尾，大 JSON 经 stdout 回传再喂 devloop_ledger 必报非法 JSON——大文件就地 python 校验+原子写（tmp+rename），不走工具参数回传。
- 2026-09-06 UX 波协调：验收检查器的输出过滤（grep ok/FAIL）会把 FAIL 明细行滤掉造成误判「空越界」——先看原始输出再下结论；显示层与判定层要分开。

- 2026-09-06 本仓库 fmt 门禁是双段（根 workspace 与 apps/cli 各自 cargo fmt）：只 fmt apps/cli 会漏根 workspace 的 crates（crates/p2p、crates/p2p-cli），pre-push 快速门禁当场拦推；跨 workspace 成员改动的收尾动作必须是两段都 cargo fmt --check。
- 2026-09-06 派单文里的会话短 id（session-7af45e36）不能直接喂 session_link_send——报「不在同一工作区」误导排查方向；先 session_link_list 拿完整 id（session-7af45e36-f764-...）再投递。
- 2026-09-06 run_code 里给 edit/write 传含 markdown ``` 围栏的多行文档内容会撞 JS 反引号模板字面量（围栏提前终止字符串，报 Expected , got ident 这类迷惑语法错）；文档 patch 一律 write 一个 python 脚本（三引号 + 转义围栏）再 bash 执行，替换点用 count==1 断言。
- 2026-09-06 中央登记的「注释行先例」只豁免 cli-parity 守卫，ai-docs-sync 的反向断言（实测命令 vs 文档条目）独立生效：上一卡新增 p2pctl 子命令只登记 tsv 不补 ai-guide 条目，守卫照样红。接手他卡遗留的守卫红先跑一遍守卫脚本读 fail 明细，不要默认是自己的改动引入。
- 2026-09-06 UI 动作的反馈若挂在全局单例连接相位上（实例：endpoint 测试连接直接订阅 acp phase），拨号悬挂时按钮无限 loading、进重连时长时间假转圈，用户感知即「点击没反应」：动作要自持生命周期（在途标记 + 终态结算 + 超时兜底），全局相位只当信号源。
- 2026-09-07 门禁命令经管道取退出码必须 pipefail：`make check 2>&1 | tail -40; echo $?` 拿到的是 tail 的 0，make 实际 Error 1 被吞成假绿（OPS1 已立规范仍再犯）；后台验收一律 `set -o pipefail` 或读 `${PIPESTATUS[0]}`，且退出码回显要贴着真命令而不是管道末端。
- 2026-09-07 接手「检查某分支并合并」类 handover 前，先 `git reflog -5` + `git worktree list` 判断并行执行会话是否已在收尾（实例：我做反向同步+跑门禁的同时，并行会话完成 ff 合并、主树验收、翻账本、删分支删 worktree，两边互相踩）；发现账本任务 doing 但远端/main 已含产物时，只做核验不做重做，避免双写。
- 2026-09-07 编辑共享 append-only 经验文件禁止「read 带 limit 只读局部 + write 全量覆写」——read limit=8 只见头部，write 写回「头部+新条目」即机械截掉全部存量（PR6 轨 known-issues.md -858 行实证）：必须不带 limit 全文读回核实行数后再追加，或用 edit 工具做纯追加锚点替换。
- 2026-09-07 主树任何写完必须立刻 commit：脏文件滞留共享主树会被并行会话的收尾打包卷进占位符提交（bae2532 实证——known-issues 截断滞留约 10 分钟后被卷入「Implement feature X」模板信息提交直落 main，截断与占位符两事故叠加）。
- 2026-09-06 派单里命令面数量口径（如「八方法」）与契约表行数（§16.1 九命令）冲突时，以契约源文档逐行数为准做全量实现，回报里明确指出口径差——少实现一个是跨轨会签红项，多实现无害。
- 2026-09-06 bash 工具返回 exit=null 且零输出 = 命令很可能压根没执行（本次 locale 提交静默丢失，收尾对 git log main..HEAD 时才暴露）：一切 commit/push 命令后必须紧跟 git log/status 机械复核，不等收尾。
- 2026-09-06 LSG1：多会话并行 cargo 高负载下，超时等待类测试（console::tests 监督时序）会偶发假红且每次挂在不同用例上；先隔离复跑单用例定性（秒级即过），别急着改代码或归因到自己模块。
- 2026-09-07 UX-E 走查前先 grep 路由注册确认目标视图真的挂载在现路由上——本任务 GroupView 是孤儿组件（routes/group-page.tsx 无消费方），群管理真实入口在 contacts 群组分区；照测试文件的路由假设走查会白跑。
- 2026-09-07 UX-E mock 数据的"随机形态"要进实现考量：mock 随机地址 70% 缺 u/t 传输前缀，带入类功能必须按"可拆才带入、不可拆只填主体"降级，走查证据里两类形态都要出现。
- 2026-09-07 协调轮 run_code 内 session_link_talk 的 talkTimeoutMs 上限只留 540s：包装层自身有 600s 墙钟，顶格 600000 必死在包装层、claimToken 都拿不到。
- 2026-09-07 协调轮 session_link_collect 的 claimToken 在 ownMessageSeen=false 时必然领不到（目标历史里还没有己方消息）；长首回合的会话不要 collect，直接隔几分钟再 talk 接力。
- 2026-09-07 协调轮 workspace_session_manage archiveSession 单会话调用会回显全工作区幂等归档清单（470+ 条），别被吓到；核对自己目标 id 在列表且活跃会话不在列表即可。
- 2026-09-07 协调轮 并行会话在共享主树各跑 pnpm 会互相打碎 node_modules（typescript 凭空消失、ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY）：验收前 CI=true pnpm install --frozen-lockfile 重建一次再跑门禁；门禁假红先怀疑依赖正被改动。
- 2026-09-07 协调轮 多分支并行收尾时 ff-only 会把合并串行化（每卡都要再反向同步一次）；main 侧改用 --no-ff 合并提交（仓库已有先例），i18n 尾部追加冲突按双侧保留解。
- 2026-09-07 协调轮 并行会话会把未提交的 skill 笔记滞留共享主树，卡住下一次 ff-merge：先单独 chore(skill) 提交保全内容，再走 --no-ff 合并解 union 冲突，绝不 stash 丢弃他人反思。
- 2026-09-07 UX-R2C 「这表单太难用/要填的太多」类反馈，先查后端已自动化能力与 UI 文案是否脱节（本次本机 agent 托管早已全自动接入，弹窗文案还在教用户手动抄 acp-console 启动输出），方向常常是删流程改文案而不是加字段做智能预填。

- 2026-09-07 UX-R2A react-refresh/only-export-components 会拦组件文件里的测试复位钩子导出（resetXxxForTest）：模块级单例 flag 的复位入口放独立非组件模块（offer-errors.ts 先例），别和组件同文件。
- 2026-09-07 UX-R2A react-hooks/set-state-in-effect 连「effect 里调用一个最终会 setState 的 useCallback」都静态点名：用 ref 中转（loadRef.current()）断开静态调用链，或回仓库 IIFE+cancelled 先例形态。
- 2026-09-07 UX-R2A 硬编码 CJK 门禁（i18n hardcoded-copy scan）连 views 源码里正则的中文匹配片段都拦（非用户可见文案也算）：用 \u 转义书写匹配片段并注释说明，测试文件不受限。
- 2026-09-07 UX-R2A EntityCombobox 是「只能选候选」型选择器，无自由输入能力：需要「选择器+自由文本兜底」时按 dial 带入同款形态（选择器回填旁边的 Input），别想着让 combobox 兼容手打。
- 2026-09-07 UX-R2A 跨卡复用纪律的可操作口径：只 import 不 edit 不算触碰红线（CopyButton/isValidFriendPeerId 均跨目录 import 复用），但 import 前确认目标文件确实通用（无本卡业务耦合）。
- 2026-09-07 UX-R2A 并行会话推进 main 后，在自己分支上跑 git diff main..HEAD 的 diffstat 会出现「他卡新增文件显示为被删」的假象：判断自己改动看 merge-base..HEAD，别被吓到去「恢复」别人的文件。
- 2026-09-06 VERIFY 轮：断言大面积假阴且 detail 全是 {} 时先怀疑驱动层（evaluate 取回函数对象未调用），再怀疑被测代码。
- 2026-09-06 VERIFY 轮：浏览器态基线断言（空→写入→清空）正式跑前作废旧 chrome profile 换全新 user-data-dir，同 profile 重跑必吃上轮残留。
- 2026-09-06 VERIFY 轮：复核前先读仓库 docs/notes/ 同类交付报告，端口/CDP 端口/mock 开关的可用配方直接白拿。
- 2026-09-07 UX-C1 波：改组件行为前先 grep 存量测试是否已锁定该行为（本次 setConfigOption 失败 toast 已被 store 层 notifyActionFailure+测试断言锁定且 promise 不 reject）——组件层再套 toast 会双 toast、改 store 会红存量测试；正确解法是组件侧做乐观值+失败回滚，提示交给既有层。
- 2026-09-07 UX-C1 波：给「匹配/总数」类计数读数做真伪判别时，先排除 store 列表在两次读取间被事件刷新（群花名册同步会重写好友列表）——读数异常先打印 store 原始数组对账，别急着判渲染 bug。

- 搜索框的命中语料必须与界面展示同源（N1 实录）：过滤命中内部串而行内渲染 i18n 摘要，用户按所见词搜索必零命中；检索文本 = 展示串 + 完整标识 + 内部字段兜底三层拼装。
- 乐观占位的派生展示必须按 status 分支：群占位 acks 为空会让「已送达 0/n」在发送中与失败态恒真（W1-01）；状态行任何统计类插值都要先问 pending/failed 下它还成立吗。
- 错误提示的复制详情不能只做在 toast 一条通道（C1）：行内/弹框内联错误要沉淀共享件（如 CommandErrorText），否则每个新对话框都会回潮成裸 p 原文不可复制。
- 2026-09-07：接到「某功能缺失」类需求先全库 grep 现有实现与测试名再动手——聊天分页早已存在（store HISTORY_SIZE + loadOlder + 后端 limit），真实缺口只是 twin 组件不同步（群流有 UX5 前插锚定、1:1 流没有）与初始页偏大；把「用户观感问题」翻译成「哪个既有环节没对齐」比重写快得多。
- 2026-09-07：改共享常量（如 HISTORY_SIZE）必须先 `grep -rn 'null, 50\|length: 50'` 找全测试断言点——9 处散在 6 个测试文件里，漏一处就是红门禁；vitest 输出里的 stderr 报错日志是错误路径用例的预期产物，别当成失败。
- 2026-09-07 IM 隐群开关轮：run_code 里手工展开 Promise.all([write,edit,…]) 多工具批次漏闭合括号即 "Expected ',' got ';'" 整批未执行——批次化改 jobs 数组存 thunk + for-await 逐个跑，结构扁平、错点单一生、失败面小。
- 2026-09-07 IM 隐群开关轮：bash 多行命令串偶发整体静默空输出（exit 0 无 stdout），同语义改单行 && 链即恢复——run_code 跑批优先单行 && 链；空输出先原样重试一次再排查，别基于空结果下结论。
- 2026-09-07 IM 隐群开关轮：pre-push hook 文案（如「纯删除引用推送，跳过门禁」）与实际 push 行为可能不符，push 成败以 git rev-parse <分支> origin/<分支> 哈希核对为准，别信 hook 打印。
- 2026-09-07 表情面板轮：新 worktree 缺 node_modules 时别 symlink 主树的（vite/tsc 缓存与并发测试互踩），直接在 apps/gui 跑 pnpm install --frozen-lockfile --prefer-offline，store 硬链接秒级完成（本次 1.6s）。
- 2026-09-07 表情面板轮：全量 vitest 偶发 1 例失败（1210/1211）而直接涉及的两个测试文件双轮全绿，原样重跑全量即 1211/1211——先按「受影响文件是否红」定位嫌疑面，flaky 单例先重跑拿结论，别急着给自己的改动翻案或补丁。
- 2026-09-08 评审页面「是否混乱」先数三样：同一后端动作的 UI 入口数（重复入口互相覆盖比字段多更致混乱）、角色/任务流是否混排、以及每列承载的表格列数——别从视觉密度入手，视觉挤往往是 IA 错位的下游症状。
- 2026-09-08 右键菜单轮：react-hooks 新规则 set-state-in-effect 禁止 effect 体内同步 setState，「量尺寸后钳制定位」类代码整段报红——修法 = 渲染期 props 调整模式（`if (anchor !== renderedAnchor) setState`）复位锚点 + requestAnimationFrame 回调里 setState（回调内合法）；用 effect 直接改 DOM 样式会被下次渲染覆写，别用。
- 2026-09-08 右键菜单轮：`A && B && C > log` 的重定向只绑最后一段 C——前面门禁的输出只进 job stdout，而流式 job_output 读一次即消费、丢中段；每个门禁段各自 `> 独立日志 2>&1` 落盘，最后统一 grep 各日志拿 RC。
- 2026-09-08 设置页微信式改版轮：视图级测试里 configGet mock resolve 后 form.reset 在 microtask 才应用，waitFor(configGet 被调) 后立刻改输入会被 reset 竞态清掉 dirty（按钮仍 disabled，点击 no-op 无任何信号）——等输入回显出加载值（value===期望值）再交互，别等 mock 调用计数。
- 2026-09-08 设置页微信式改版轮：closest(".items-center") 是自包含匹配——目标元素自己带 items-center 类时返回自身（textContent 为空制造「行里没内容」假象），要用更特异的 div.justify-back/div.items-center 跳过自匹配。
- 2026-09-08 设置页微信式改版轮：新增视图级测试必须整备全部隐式上下文——分节常挂后 AppearanceCard 要 ThemeProvider、入口行要 MemoryRouter（useNavigate invariant），单卡测试时代不暴露；报「occurred in <组件X>」先查 X 的 hook 上下文缺谁。
- 2026-09-08 设置页微信式改版轮：jsdom 无 scrollIntoView，未 stub 时 focusFirstInvalidField 在 RHF onInvalid 回调内抛 TypeError，onInvalid 里排在它之后的 setState 静默不执行——表现是「保存点击无反应」而非测试报错；新测试先抄 settings-focus-error 的 `HTMLElement.prototype.scrollIntoView = vi.fn()` setup。
- 2026-09-08 llm-share UX 轮：表单「区段级错误展示」绝不能硬编码单一错误键（本次 SpareRows 把 errors.spare 恒显为 errSpareRequired，真实 errPositiveInt 被吞）——必须透传真实错误键；测试断言要区分具体错误文案，只数 alert 数量或「包含任一错误」都抓不到这类显示层撒谎缺陷。
- 2026-09-08 llm-share UX 轮：把常驻表单改成「按钮展开」后，测试助手必须 async 化（CTA/表单随异步加载出现，getByTestId 有竞态，统一 findByTestId + await）；改组件交互形态时先 grep 渲染同一组件的其他测试文件（本次 peer-id-field.test 渲染 AllowlistPanel，漏改了一轮才发现）。
- 2026-09-08 llm-share UX 轮：并行会话推进期间主树 main 会前移——feature 收尾先 git merge main 反向同步并重跑门禁；ff-only 合并前用 git rev-parse main origin/main 核对双指针一致（本次 main 会话中途从 a40e464 前移到 7772200）。
- 2026-09-08 设计稿轮：设计类需求必须先读已实现 GUI 的真实令牌与组件再喂图像模型——色值取 index.css（#07c160/#fa5151/10px ring 卡）、结构取目标页面组件、文案取 zh-CN.ts 原文；第一版凭「通用 SaaS 风格」自由发挥被用户点名返工（要根据当前已实现 GUI 风格、参考现有页面功能，别瞎发挥）。
- 2026-09-08 设计稿轮：run_code 的 JS 模板字符串会吃 bash 的 ${...}——`${OPENAI_API_KEY: -6}` 触发 JS 插值报错、`${#VAR}` 被解析成私有字段；shell 逻辑一律写成脚本文件再 `bash x.sh` 执行，run_code 里只放无 $ 变量的简单命令。
- 2026-09-08 本地 ACP 回环轮：线协议两端各自单侧测试全绿 ≠ 真机能通——acp-console 握手/泵是裸 ndjson 字节、acp-agent 是 varint 帧，同仓库两侧测试各自为政从未跑过真传输，接缝缺陷靠「跨进程真传输的集成测试」（两端都是真件）才能拦，mock 夹具只能证单侧行为。
- 2026-09-08 本地 ACP 回环轮：mock 夹具若按「实现的现状」抄写而非按「契约的另一端」实现，缺陷会被夹具固化成永绿的假象（AgentMock 裸 read_line 复刻了 console 的裸写错误）；写 mock 先问协议文档怎么说。
- 2026-09-08 本地 ACP 回环轮：常驻部署的二进制（launchd/侧栏 sidecar）会与仓库漂移——「功能缺失」先 diff 运行实例与仓库 HEAD（新端点 404、新 flag 不识别都是老化信号），再决定写代码还是先部署。
- 2026-09-08 本地 ACP 回环轮：临时/测试实例禁止写用户级共享单槽文件（如 ~/.dsh/acp/local-agent.json），必须留 --descriptor-disabled 式禁用开关，否则冒烟覆盖生产描述、GUI 读到死端口。
- 2026-09-08 本地 ACP 回环轮：worktree 内 bash grep 偶发无输出（疑符号链接/路径解析问题），排查别死磕一条命令——换 SDK grep 工具或 python 逐行扫描立刻现形。

- 2026-09-08: run_code 的 JS 模板字符串里写 markdown 代码块必须转义反引号；单引号字符串不能跨行——大文档用 bash heredoc（带引号定界符）写入最稳。
- 2026-09-08: 并行会话会同窗合并 main——每次 push 前先 fetch + rebase；ff-merge 被拒唯一动作是回 worktree rebase 后重试。
- 2026-09-08: itest 夹具里 use acp_agent::{a2a, ...} 会影子化外部 crate 名 a2a——用 as host_a2a 别名。

- 2026-09-08（A2A3 波）页测试断言 sonner toast 文本前必须挂 <Toaster />（render 包裹，friend-invite-row.test.tsx 先例）；beforeEach toast.dismiss() 防跨用例残留。
- 2026-09-08（A2A3 波）对话框「打开瞬间播种」的条件三元要逐分支核对默认值：editing?.visibility === "public" ? "public" : "private" 把编辑 null 分支也落成 private——创建态默认值被编辑态表达式吞掉，测试断言 POST body 才现形。
- 2026-09-08 W5b：run_code 内多行补丁脚本绝不在 JS 单引号串里嵌 python 多行转义——JS 会把 \n 还原成真实换行打断 python 字面量（SyntaxError EOL），且 stdout 被 tail 截断后静默无感知；唯一可靠姿势 = write 工具落脚本文件（python 内一律 chr(10) 拼接）再 bash 执行，每步 replace 前后 assert count。
- 2026-09-08 W5b：测试文件逼近 300 行红线后再拆，成本（#[path] 模块编译、crate:: 作用域、E0255 同名冲突、line-limit 实红重跑全量门禁）远超开工前先定文件骨架；多用例任务先按职责分文件再落用例。

- [2026-09-08 A2A波] 验收判据禁用管道尾命令：`make check | tail -4 && echo GREEN` 的 GREEN 是 tail 的退出码——已两次产出假绿；必须直读 RC。
- [2026-09-08 A2A波] 子代理额度死亡=interrupt+显式 provider/model 重派单，WIP 在 worktree 天然持久；接续前先查产物（commit/dirty）避免双写。
- [2026-09-08 A2A波] 共享 .devloop/loop-state.json 会被并行波整体重构（72卡→7卡），波次收口记录必须落 docs/notes/ 才是持久真相源。

- 2026-09-09 生图设计轮：同一 worktree 路径被两个会话并行用于生成两套不同设计稿——生成类任务的落盘文件（PNG/脚本/specs）大多处于未跟踪状态，git status 完全看不出互撞，specs.py 被整体覆盖、PNG 按文件名互相顶替；判归属光看 git worktree list 不够，进入目录先找 COLLISION/FINALIZE 类 note 文件并 diff specs 内容。预防：worktree 路径必须带任务唯一后缀（gui-mockups-p2p / gui-mockups-dsh 这种），发现撞车后留 note 搬迁而不是抢目录。
- 2026-09-09 生图设计轮：session_link_talk 返回空回复或超时 ≠ 工作会话死了，多数是它正在跑长任务（生图单张 60~130s、一整套 20~60 分钟）；磁盘产物（文件 mtime、results/log json）才是进度真相源。接管前先查产物，接管时在目标目录留 FINALIZE note 声明归属防双写——本次留 note 后顺利接管了无主套图。
- 2026-09-09 生图设计轮：图像模型 edits 端点的内容漂移两种形态——(a) 输出变成参考图的近似复制品（06 首版把 Agents 页画成了总板），(b) 自由发挥丢弃 spec 关键分区（05 丢了邀请卡）。重试通常可修复（06 重试 1 次即正），验收必须逐张读图比对 spec，不能只看风格。
- 2026-09-09 生图设计轮：写探测/工具脚本时模块级副作用会在 import 时重跑——probe 脚本被 import 复用导致 3 次多余 API 调用；一切有副作用的脚本主体必须包 `if __name__ == "__main__"`。
- 2026-09-09 生图设计轮（负责人）：派发设计稿任务书必填「画布比例/尺寸」字段，从产品形态与外壳文档推导（Tauri 桌面 min 960x600 → 1536x960@16:10），不要等用户看到方形稿才纠正。
- 2026-09-09 生图设计轮（负责人）：页面类出图首选 generations + prompt 内联 token 锚定 + 反设计板负约束；edits 参考图通道留给需严格贴参考的场景——03/04/06 改道 generations 后全部 1~2 试命中，且绕开 edits 在大尺寸下的高频 524（multipart 更重、源站更慢）。
- 2026-09-09 生图设计轮（负责人）：多会话共享同一个不稳定上游（生图中转）时必须全局互斥——工作会话的「并发赛马」与接管方的串行重试互相加剧 524；协调者接管资源前先 interrupt 对方的生成行为再开跑。
- 2026-09-09 通讯录资料互通轮：全量 vitest 与 cargo 大编译并发会把 runner 打出 worker timeout 假红（同树 6 failed 全超时，静机重跑 1334 全绿）——跑门禁前让机器静下来，假红先静跑复现再排查。
- 2026-09-09 通讯录资料互通轮：往接近红线的文件加功能，先想好拆分位再动手——lib.rs 装配块整体迁 boot.rs 比逐行抠注释体面（同 crate 允许 impl 分文件），register 助手顺带消掉三段重复注册。
- 2026-09-09 通讯录资料互通轮：后台 job 会被会话续接清掉（job_list 里 unknown），重启的编译因缓存秒级完成——续接后先 job_list 再决定重跑，别假设长编译要重头来。
- 2026-09-09 A2A聊天修复轮：TS 契约写 Promise<unknown> 而消费端用 `as` 强转成具体类型再取字段，是定时炸弹——A2A transport resolve undefined 时 composer 取 report.delivered 直接 TypeError，且发送成功也误报「发送失败」。「unknown 进出 + as 强转取字段」的代码，修法是把 unknown 顶到判定函数做结构化守卫，强转只会把崩溃点藏到别处。
- 2026-09-09 A2A聊天修复轮：报错文案会撒谎——用户看到的「发送失败 undefined...」其实是三层叠加（通道没接线→真实错误被 catch 吞掉→composer 崩溃产生误导性 toast）；修 UI 报错先还原「错误传播链上每一层各自吞了什么」，最外层文案只是冰山尖。
- 2026-09-09 A2A聊天修复轮：给 fake transport 写回归测试时，测试替身的时序要贴真传输（异步 decodeFrame、microtask 分帧）——同步读 store 断言会踩竞态假红；统一 flush（setTimeout 0）再断言。测试先行还顺手揪出三个 store 真 bug（create 应答与入簿窗口丢通知、agent 消息缺 messageId 戳破坏去重、createTask 漏 lastError 留痕）。
- 2026-09-09 A2A聊天修复轮：并行会话对 main 的高速追加下，ff-merge 是重试循环：worktree 内 rebase main → force-with-lease 推分支 → 主树再 ff-only；ff-only 失败零损失（ref 没动），merge commit 才会让历史分叉难看。
- 2026-09-09 authz A1 轮：写 Deny 路径断言前先核对角色权限并集——本仓内建阶梯 friend⊂guest⊂operator⊂ally 的并集恰为 §4 全部九 key，绑 ally 后查任何已登记权限都是 Allow；MissingPerm 断言要选所绑角色真不含的权限（如重绑 friend 后查 llm.borrow）。
- 2026-09-09 authz A1 轮：测试里 --expires 别用小 unix 秒（123=1970 年，瞬时 Expired），要过期用过去时刻并明示，要有效用 9_999_999_999 或 None；判定层按 now>=expires 含边界即拒是对的，别把测试预期写反。
- 2026-09-09 authz A1 轮：p2pctl 的 --data-dir 是每个叶子子命令的参数（clap Args 内联），不是全局参数——`p2pctl --data-dir X authz ...` 报 unexpected argument，必须 `p2pctl authz ... --data-dir X`。
- 2026-09-09 authz A2-LLM 轮：workspace exclude 的装配面（apps/gui/src-tauri）字面构造 workspace crate 的公共结构体，却不被 cargo clippy --workspace 覆盖——给这类结构体加字段/改类型必炸 workspace 外消费者且门禁全绿看不见；跨 exclude 边界发新能力优先 builder/setter 注入（with_gate1_authz 先例），构造签名与字段形状保持冻结。
- 2026-09-09 authz A2-LLM 轮：clap derive 的元组变体 `Variant(SubcommandEnum)` 要求内层实现 Args 而非 Subcommand，E0277 报错点却在 mod.rs 注册行——subcommand 枚举必须用具名字段形式 `Import { #[command(subcommand)] command: ... }`（与 A1 记的 --data-dir 叶子参数规律互补：注册形状由 clap 侧决定，不由直觉）。
- 2026-09-09 authz A2-LLM 轮：泛型参数擦成 Arc<dyn Fn> 闭包字段时，impl 块要一次补齐 C: Send + Sync + 'static 三界（Clock 本体两界都不带），少一个都是 E0310/E0277 连环；先算清擦除目标的 auto trait 要求再写 impl 头，省两轮编译。
- 2026-09-09 authz A2-LLM 轮：「上游 API 不够用」先翻它的 storage/底层次模块再报缺——p2p-authz ops 层没暴露绑定读，但 store::load_bindings 是 pub，import 幂等（跳过已绑定）全靠它表达，零接口新增；报缺接口前先证明公开面真表达不了。

- 2026-09-09 给共享枚举加变体 = 破坏同 crate 一切非穷尽 match；当 match 所在文件属于禁改面（如 a2a/**）时，正解是新增外层包装枚举（如 GatedDecision 内含 Decision）或纯新增函数，让旧类型形状冻结——「不改别人的文件」要从类型形状层面兑现，不只是不碰路径。
- 2026-09-09 `cargo clippy --workspace` 只对成员做 lint，path 依赖只编译不 lint：成员面板的 `-D warnings` 不会因依赖 crate 的存量 warning 而红。判「零增量」别看单次 exit，要与基线树同命令跑一遍再双向 diff error 集合。
- 2026-09-09 准入/判定加闸后既有集成测试大面积红的正确姿势：测试夹具按迁移映射（§9）写授权态模拟「import 后稳态」，需要走被闸路径的用例显式声明更高角色——比给产线加「测试旁路开关」忠实于设计，也把新语义写进了测试名里。
