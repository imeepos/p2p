# Lessons

<!-- 一条经验一行。格式：当 X 发生时，修复是 Y。skill 没提前警告我。 -->

_none yet — be the first._
- 2026-09-04：当要在带机械校验区间的文档（如 p2pctl-ai-guide 的 AI-DOCS-SYNC 区间）回填内容时，修复是区间内只加纯文字、新章节放区间标记外——区间正则把散文里的 `--词`/`<大写>`/`[大写]`（连 `[CAPTURE_PERMISSION_DENIED]` 这种错误码）都抽成参数与 `--help` 双向比对，多一个 token 整个门禁红。
- 2026-09-05：Tauri app.emit 在前端 listen 装好之前发出即永久丢失（事件不缓存），E2E 断言"前端已感知"前必须先等就绪门（监听装好后向前端日志写标记行，脚本轮询到标记再开写）。
- 2026-09-05：集成不熟的依赖库先 cargo fetch + grep registry 源码确认真实签名再写码，别凭文档记忆写完再修（notify-debouncer-mini 0.5 三个假设全错：new_debouncer 只有 2 参、DebouncedEvent.path 是单数、Debouncer<T> 泛型是 Watcher 不是 handler）。
- 2026-09-05：任务卡的"触及路径白名单"要与需求联动自查后再动手：给中央注册表加条目必然改它的守卫测试（page-registry.test.ts 路由数清单 8→9），白名单漏列守卫测试时唯一解是「最小修改 + 回报显式标记例外」，硬守白名单只会让门禁假红、A1 必挂。
- 2026-09-05：run_code 用模板串写长文件时，漏闭合反引号/转义混乱会在"解析 program"阶段就炸（Expected ',' got ';' / Unterminated template），与目标文件内容无关；先写 30 行小片验证模板串本身，再写全文件。
- 2026-09-05：工具类管道命令 `make check 2>&1 | tail` 的 exit code 是 tail 的（恒 0），make 失败被吞；一律 `set -o pipefail` 或 `rc=$?; echo RC=$rc` 显式回传（验证两次才敢报绿）。
- 2026-09-05：pnpm test -- --run <path> 对 script 形 vitest 并不能按路径过滤（全量照跑），子集调试用 `pnpm vitest run <path>`。_
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
- 2026-09-04 N2：git stash pop 或外部脚本改写文件后，edit 工具必报 file changed
  since it was read——先重读再改，别凭记忆构造 old_string。
- 2026-09-04 N2：并行会话会在你验收窗口内推进 main（本次 ai-guide 会话把 main
  从我的合并点 ff+merge 到 214c41f）；ff 合并后尽快 push main，回报合并 hash
  用自己的合并点并注明 main 已前进到含它的后继提交。
- 2026-09-04 N1：文档与实现的漂移是真实发生的（cli-guide.md §5 曾写 --peer-id/--name，
  实际 clap 参数是 --peer/--nickname）。凡"命令面/接口面"文档必须配机械同步门禁
  （实测 --help ↔ 文档双向比对），手写完事必漂。
- 2026-09-04 N1：合并到 main 的收尾动作（worktree remove / branch -d）做完后立刻
  复核交付物仍在 main 且 hash 可达——并行会话可能同时推进 main 指针，别拿最后一条
  log 行当自己的合并提交。
- 2026-09-04 IM-T48：视觉走查驱动脚本先跑探针模式（单页单断言核对取值层级/选择器/等待策略）再扩全量
  矩阵——直接上全量脚本，错层 bug 让三轮全量白跑。
- 2026-09-04 IM-T48：mock 环境的状态覆盖上限开工前查清（p2p GUI mock 心跳只连 randomPeerId，好友永不
  连接、chatSend 无失败路径——delivered/failed/them 气泡在 mock 下不可达），能进 vitest 的状态别指望
  浏览器走查凑出来。
- 2026-09-04 IM-T48：对比度/几何这类"离线可算"的证据比运行时截图更硬，但计算脚本必须带锚点自校验
  （白/黑=21.0、#767676/白=4.54），否则会产出貌似精确的错数字并写进提交信息。
- 2026-09-04 R1：run_code 里成功的 bash 调用也可能吞掉 stdout（本次 heredoc 写文件与链式命令两次返回空）——写文件/验收后必须独立回读验证（wc/bash -n/grep），空输出既不能当失败也不能当成功证据。
- 2026-09-04 R1：进程内「两个 Store 实例指向同一数据目录」等价复现跨进程并发写竞态（各自内存态、无共享锁，修复前 B 启动时空簿会整簿覆盖 A）——跨进程丢写类回归先用双实例单测毫秒级锁住，跨进程 E2E 只做终验。
- 2026-09-04 IM-T50：主树 make check 是并行轮的共享资源，load 40+ 下 vitest 超时与 cargo 子进程竞态都是环境病——先隔离定性再错峰重跑，别对着环境红改代码。
- 2026-09-04 IM-T50：被协调者通牒/接管时，第一动作是只读核查自己工作区状态并秒回事实清单（已交付提交/未提交面/后台任务），迟到的沉默会被误读成无进展。
- 2026-09-04 IM-T50：known-issues 登记过的坑（zustand 选择器 `?? []` 快照不稳）写测试时没回忆起来又踩——写涉及 store 的代码前先 grep 一遍 references 再动手。

- 2026-09-05（ACP2 轮）：run_code 的 JS 里嵌大段 Rust/TS 源码用模板字面量会被内容打断——两个假错误（"Expected ',' got ')'"、数组里裸标识符 ReferenceError）排查各耗 20 分钟，与真实 bug 无关。稳法：content 用行数组 + join 换行构造，数组内禁止留占位裸标识符，写完立即 wc -l 核行数。
- 2026-09-05（ACP2 轮）：并行会话共用 /tmp 撞日志文件名——/tmp/acp-check.log 被并行 ACP3 会话的 cargo 输出整份覆盖，一度误判成自己的构建在编别人的 crate。对策：临时日志一律带卡号/会话号前缀（如 acp2-a2-*.log）。
- 2026-09-05（ACP2 轮）：DSH 会话重启会 TERM 整个后台任务树，nohup+disown 也逃不掉；长门禁（make check 全量 30 分钟+）对策=按 make 目标分片前台跑（每片 timeoutMs 10 分钟级），cargo/pnpm 增量缓存让被杀重跑只补剩余，各片 EXIT 落日志文件断点续跑。
- 2026-09-05（ACP2 轮）：make check 十个门禁目标各自幂等可独立跑（gate-tests/version-check/fmt-check/line-limit/clippy/test/gui-check/panic-hygiene/cli-parity/ai-docs-sync），分片全绿等价整跑；重活 clippy/test 靠前次被杀留下的增量缓存，二跑 0.4s/3min 收官。
- 2026-09-05（ACP2 轮）：git commit -m 的多行 message 经 JSON.stringify 进入 bash 双引号后 \n 是字面反斜杠 n，不换行；多行提交信息一律写临时文件用 -F 传。
- 2026-09-05（ACP2 轮）：底座契约缺口——p2p ProtocolHandler::handle(stream) 不暴露远端 PeerId，需要 per-peer 鉴权的上层 app（acp-agent）只能订阅 Node 事件维护在线集：恰一 peer 在线才归属、多 peer 歧义 fail-closed 拒绝（session.rs+peers.rs 已实现并测试），待底座流分发层把 peer 传进 handler 后解除；做 ACP4/继续连时别在这个边界上再踩一遍。

- 2026-09-04 IM-T49：rustup 升级（clippy 1.98）会让已合入 main 的存量代码突然门禁红（needless_borrow/suspicious_open_options 新 lint），与在途改动无关——验收红先看报错里的 lint 名与 clippy 版本再定性，零行为机械修复按 IM-T47 先例随当前分支独立 fix 提交并在报告披露。
- 2026-09-04 IM-T49：残留 worktree 的增强测试拖 17 天不评估，API 已漂移四处（Chat::send 加参/WireEnvelope 加字段/命令加参/端口夹具），适配成本随时间只涨不跌——盘点类清理任务要早办，评估结论本身就是适配点的清单。
- 2026-09-04 CLI 演练：共享主树多会话并行下，测试会话进行中 main 会漂移（本会话 004b441→ffc0f0d 且中途出了热fix），报告必须锚定「实际构建所用的 commit」并对期间合入的相关修复做同异性辨析，否则缺陷归因张冠李戴。
- 2026-09-04 CLI 演练：ssh 远端嵌 python 单行做 JSON 解析，双层引号每嵌一层丢一档转义（\"message\" 到远端成 [message]）——远程侧解析用 tail/grep 最简形态，或把解析脚本放本地管道远端只出原始行。
- 2026-09-04 CLI 演练：IM-T49 已登记的「| tail 吞退出码」在 bash 侧再踩（cargo build | tail 后 $? 取到 tail 的 0 假绿）——构建/发布类命令后判产物存在性与 file 格式才是权威验收。
- 2026-09-04 GC3：派发大型多文件任务给 dispatch_task 前先拆掉「冷构建」因素（构建单独后台跑、代码产出单独派），600s 墙钟只够纯编辑型工作；超时后先查产物再接管，半成品质量好就接着用，别推倒重写。
- 2026-09-04 GC3：开工前 git worktree add 要单独一步跑且给足超时，跨步骤复合命令被中断会留下「分支已建、目录半成品、worktree 未注册」的三态脏局面，恢复顺序=删目录→删分支→重来。
- 2026-09-04 GC3b：vitest 4 的 test options（timeout 等）在第二参 it(name, {timeout}, fn)，放第三参（旧版 number 位置）过不了 tsc；修「负载型假红」时同病守卫要 grep 全库找齐一起治（IPC 守卫修完后 i18n 扫描守卫立刻顶上暴雷），否则验收口径（连续两遍全量）永远凑不齐。

- 2026-09-04 U1：验收命令的调用形态就是测试用例——自测覆盖了 --keep 形态、验收跑无参形态，set -u 一行崩全盘；先读验收命令再反推脚本必须吃下的全部调用形态。
- 2026-09-04 U1：head -c 8 经 xxd -p 产出 16 个 hex 字符，与 8 字符 magic 常量做全等恒假（60 断言里唯一全崩的一条）；模板里 grep 前缀匹配是有语义的，抄模板要抄语义而不是自创写法。
- 2026-09-04 U1：链式命令里 cd 失败后续命令在原 cwd 继续跑出假结果（在主树里读出 branch=main 误判 worktree 异常）——链首 set -e 并显式回显 pwd 再做状态判断。
- 2026-09-04 U1：并行会话活跃仓库里 main 高频前进、worktree 注册可能被他人动过——每个 git 操作后立即验证真实状态，合并前 fetch 反向同步，异象先查 worktree list 与 reflog 归因再动手。
- 2026-09-04 U1：run_code 模板字符串里内嵌含美元花括号的 bash 内容必炸（TS 插值与 bash 展开双层打架，本日三次 parse error、一次 commit message 吃掉形参）——提交信息走文件加 commit -F，脚本内容避开该写法。

- 2026-09-04 U2（gui-updater 轮）：60s 超时连环杀进程的根因是 ext512 外置卷小文件 I/O 病态慢——clone 30MB 仓库 8m48s、push 本地 1m25s、rm -rf 带 node_modules 的目录必然超时；已知条目只记了「被杀」现象，本轮补根因与对策：重活全部显式 timeoutMs（10 分钟级）或丢 background，主战场搬到内置卷（/tmp）clone。
- 2026-09-04 U2（gui-updater 轮）：本日两次 worktree add 成功 checkout 后 `.git/worktrees/<name>` 元数据消失（git worktree list 不显示、worktree 内 git 命令报 not a git repository），一次还伴随 checkout 缺整个 crates/ 目录；同仓库其他会话的老 worktree 完好，疑与并行会话 git 操作/外置卷异常叠加有关。对策：worktree 创建后立即 `git worktree list` 验证；元数据消失时只清目录+prune，绝不在残骸上继续干活。
- 2026-09-04 U3（gate-braces 轮）：分析时刚写出「模板串里 ${ 会插值」的警告，下一步自己仍用模板字面量嵌修复脚本，当场 ReferenceError——识别出转义风险的那一刻就切换写法（行数组/write 工具），不要让惯用模板串活到下一行代码；另：grep 命中按行报告，一行多处 `$var` 会漏数（本次 15 行实为 16 处），替换后必须用同一正则复查清零兜底。

- 2026-09-04 U1：端点就绪 ≠ webview 桥就绪——tauri 控制端点先写、React 桥后安装，重载下差几十秒；就绪探针语义必须是「桥有应答」而非「命令退出 0」：初始路由 dashboard 未注册属合法应答（exit 1 + PAGE_NOT_REGISTERED），只有 PAGE_TIMEOUT 才是桥没起来。判「命令成功」会让就绪环永不满足。
- 2026-09-04 U1：脚本里 kill 后台子进程后，其信号终止状态可经异步 reap 泄进脚本退出码（绿灯运行被调用方 && 链误断）——disown + cleanup 内显式 wait 归零 + 成功路径显式 exit 0，三板斧跨 bash 版本确定。
- 2026-09-04 U1：并行会话会扫提交主树未跟踪文件——主树里的 scratch 测试副本（ui-regression-scratch.sh）被顺手指令提交入库（c92d983）；测试副本要么放 /tmp 要么用完立刻删，主树留过夜就会被别人"帮忙"入库。
- 2026-09-04 U1：tauri custom-protocol 构建把前端 dist 嵌进二进制——pnpm build（先清空 dist）与 cargo build 并发时，嵌进去的可能是空/半成品 dist，症状=webview 加载空页、页面桥永不安装且无任何报错；修法=重跑 cargo build --features custom-protocol 重嵌。诊断入口：~/Library/Logs/com.p2p.console/{p2p-console,frontend}.log（应用日志不走 stdout，脚本里 tail gui.log 常为空）。
- 2026-09-04 U1：回归脚本的就绪/探针类修复要与并行会话的同类分支对齐格式（同文本改动 git 自动合流）——动手前先 grep 别人未合并分支对同一文件的改动。
- 2026-09-04 R7：rsync 目录到远端构建机必须 --exclude target——漏排一次就把本机 Mach-O 盖掉远端 ELF，且 rsync -a 保 mtime 让 cargo 误判最新不重编（真机回归两次被坑，判据=file 格式而非存在性）。
- 2026-09-04 R7：测试辅助函数返回 Future（如 wait_until 调用方需 .await）时，漏 await 不报错只降级为「断言永远即时通过」——失败行号指到无关断言，先 grep 编译警告 unused Future 再读断言。
- 2026-09-04 R7：Copy 类型（PeerId）逐字段改造时 clippy clone_on_copy 会连环冒出——改签名前先查类型是否 Copy，一次改净。
- 2026-09-05 T19：homebrew bash 5.3.9 起 `$var（`（变量名紧邻多字节字符）展开会把首字节并入变量名（f\357），set -u 直接 unbound variable；/bin/bash 3.2 无此行为——验收命令 PATH 前置 /opt/homebrew/bin 时 make 配方里的 `bash` 解析到 5.3.9，同脚本两个 bash 版本行为分歧先查 `which bash`。门禁/脚本里 `$var` 紧邻非 ASCII 一律写 `${var}`（version.sh 缺文件路径、FAIL 分支文案全是雷区）。
- 2026-09-05 T19：cli-parity 门禁 `if [ ! -x p2pctl ]` 惰性构建 + 验收命令全局 CARGO_TARGET_DIR → 产物落 /tmp 而脚本找 apps/cli/target（空则当场崩）；且存在旧二进制时无条件信任——陈旧二进制把另一波漏更文档的登记债整个掩盖（TSV 新行 + 旧二进制 = 假绿，删二进制才显形）。接手共享门禁前先查 `ls -la apps/cli/target/debug/p2pctl` 的时间戳对不对得上当前源。
- 2026-09-05 ACP5：验收 && 链中「cmd | tail N」的退出码取 tail 恒 0，遮蔽上游失败（本轮 clippy 假绿：INNER=0 但 CLIPPY_EXIT=101）；写验收链改用 PIPESTATUS 或拆步判定，复核旧日志 grep build failed/error 残留。
- 2026-09-05 ACP3：开工前必读清单要含 skill references 的「当日条目」——worktree 被并行 prune/超时杀 checkout 的坑当日已有两条同族记录，先读能省两轮重建；把与本卡操作同形的条目（worktree/长构建/转义写文件）过一遍再动手。
- 2026-09-05 ACP3：验收门禁红灯先做「域归属三步定性」——git diff origin/main HEAD --stat 看红灯域文件是否在自己 diff 里、查红灯读的输入是源码还是工作树陈旧产物、隔离复跑最小面；三步都干净就原样上报协调者，不代修邻域（并行会话可能正在改同一处）。
- 2026-09-05 ACP3：对下游不可见的传输语义（EOF/半关闭/窗口更新时机）要在设计评审时就写探针用例锁行为——yamux 批量窗口更新饿死写侧是集成测试随机挂死才暴露的，探针前置能把 3 小时定位压成 10 分钟；且探针纠偏过一次源码静读的误判（半关闭其实可用）。

- 2026-09-05 ACP4：tokio 单任务要同时 select 多个流又要在分支里 &mut self：把 select 收敛到一个 next_event 辅助函数（只借 receiver），分支体再借 self，借用冲突自然消失；跨模块拆 impl 时子模块可访问父模块私有字段。
- 2026-09-05 ACP4：集成测试无限挂死的标准排查——先 pkill 全家（cargo/测试二进制/桩进程），再逐测跑 + 后台任务落盘状态文件（START/END/rc+时间戳），挂点一目了然；整包 600s 超时跑只会烧时间。
- 2026-09-05 ACP4：测试断言失败先用探针对照真实值再定性——本轮监狱目录断言拿错 peer（桥是对的）、replay 后撞上 initialize 回声行（合法透传）都是断言错；但真正的桥缺陷（代答缺换行、早期行真空期丢失）也伪装成测试挂死，两者必须靠探针证据区分。
- 2026-09-05 G2：agent bash 通道里多行 shell（heredoc/跨行引号串）高频翻车（本轮两次 syntax error）——git 提交信息的稳法是 tools.write 临时片段 + git commit -F 片段 + rm；「多行命令用 join(' && ') 拼接」会把 heredoc 体逐行粘上 && 直接报废，长命令一律拆步或落文件。
- 2026-09-05 G2：长任务期间主树会漂移（本轮开工 fetch 时 main=895c3a4，收尾已到 c1ade46，58 文件含 GUI 与 scripts/check 本身）——开工 fetch 不够，收尾四步前必须重查 main；ff-only 失败走「worktree rebase main → force-with-lease 推分支 → make check 全量重跑（门禁脚本变了旧绿不算数）」，无文件重叠的 rebase 十秒内完成，别怕。
- 2026-09-05 G2：给 NodeEventJson 判别联合加事件类型的全部下游落点清单——event-meta.ts 的 BADGE_VARIANT 与 EVENT_TYPE_KEY 两张 Record<NodeEventType,…> 表 + eventSummary switch + lib/event-text.ts 的 describeNodeEvent switch + 双 locale 的 events.types/events.summary 子树；漏一处 tsc/gui 门禁就红，按清单逐点补齐再跑门禁。
- 2026-09-05 G2：react-hooks/set-state-in-effect 门禁下视图初始取数的仓库既定写法是「const load = useCallback(() => { ipc.x().then(setState).catch(收尾) }, [])」+ effect 直接 load()；async 函数体内 setState 再在 effect 里 void 调用会被新规则拦（diagnostics-view.tsx 注释即此 idiom 的权威示例）。
- 2026-09-05 G2：测试文件同样受 300 行纪律（存量 310/314 不构成豁免）——mock 单例测试拆文件时事件收集器必须带 reset() 并在 beforeEach 调用，否则同文件用例间事件计数互相污染（本轮 3 个假红全源于此）；拆分是纯 test refactor 单独成提交。
- 2026-09-04 AI 试运行：文档驱动操作撞上「两套身份」类系统时，把每次失败报错当拓扑探针用——Failed(快速) vs Pending(超时) 的差异本身就指明对端身份不符还是网络不通。
- 2026-09-04 run_code：bash 惯用法写进 JS 模板串必须转义美元花括号（PIPESTATUS 被 JS 当插值求值直接 ReferenceError），含反引号的内容更要用数组 join + heredoc 落盘。
- 2026-09-04 ACP6b：run_code 给 tools.write 传整文件内容时模板字符串可能炸出莫名 lexing error（Expected ',', got ';'），与内容里反引号/插值无关——单引号行数组 + join("\n") 是稳态（191 行中文协议文件一次通过）。
- 2026-09-04 ACP6b：异步回放面的断言 helper（取 mock 权限请求 id、等能力卡出现）必须内部 waitFor，同步取值比帧到达早半拍直接空引用；helper 返回 Promise 让调用方 await，别为了调用简洁做同步版。
- 2026-09-05：给被大量既有测试裸渲染的视图加 useNavigate，会把所有无 Router 包裹的旧测试一次性打崩（G3 一加 79 失败）；App 根是 HashRouter 时改 window.location.hash 跳转零测试改动，新路由测试也用 HashRouter + window.location.hash 驱动。
- 2026-09-05：zustand 页面接第二个 store 订阅同一 ipc 事件总线后，既有测试的 onNodeEvent 替身若写「handler = current」单槽会顶掉前者、chat_message 断言全超时；替身必须改数组 push + 循环投递（真实事件总线一对多），且数组不许 beforeEach 清空——store 订阅有模块级 latch，清空后后续测试无人收事件。
- 2026-09-05：eslint react-refresh/only-export-components 禁止 .tsx 组件文件同导出纯函数（G3 在 group-list.tsx 导出 orderedGroups 被红）；纯函数下沉同名 .ts。react-hooks v7 的 set-state-in-effect 禁 effect 内同步 setState——面板类组件用「open 才挂载」的父级条件渲染替代打开时重置状态。
- 2026-09-05：vitest 的 vi.fn<T>() 泛型参数个数必须与 mockImplementation 实参个数一致（tsc TS2345）；带参 mock 声明写成 vi.fn<(groupId: string, memberId: string) => Promise<R>>()。
- 2026-09-05：断言用 getAllByTestId(/^prefix-.+$/) 正则会把容器 testid（group-member-panel/group-member-list）一并匹配导致假红；正则按值域字符集收窄（base58 无 l/i 恰好排除 panel/list）或行级 testid 用独立前缀。
- 2026-09-05：GUI 单群操作无契约命令时（v1 无 group_disband），用已有 owner-only 原语组合出操作面（逐个 groupKick 全员=解散入口）并在确认文案明示语义降级，交接报告单列遗留——比留死按钮更利于后续契约加法时切换。
- 2026-09-04 G1：pub(crate) 类型不能 pub use 出 crate（E0365）——契约模型要跨 crate 消费就直接声明 pub，别先 pub(crate) 再 re-export。
- 2026-09-04 G1：300 行红线 + rustfmt max_width=100 下，链式调用/多参签名按 fmt 后行数计预算（fmt 展开动辄 +30%）；跨模块 `impl 同crate类型` 是合规分文件手段，文件头注明"行数红线再平衡"。
- 2026-09-04 G1：并行会话高频合入 main 时 ff-only 失败是常态——rebase → 重跑 clippy+line-limit+定向测试 → 立即重试，整套动作一分钟内连惯做完，拖久了又漂。
- 2026-09-05：当 crates.io 依赖凭记忆写小版本号时，修复是先查 sparse index 真实版本列表——`yrs = "0.6"` 解析到 2021 年远古 API（PrelimMap/Transaction 单数形态），现代 yrs 是 0.27+（TransactionMut/ReadTxn trait 形态），API 完全对不上。
- 2026-09-05：当 E2E 断言「脚本末行 == OK 标记」时，修复是 grep 标记而不是 tail -1——trap 清理函数在 OK 行之后还会输出 Terminated 等系统噪声，tail 会误报。
- 2026-09-05：当分支合并期间 main 被并行会话连续推进导致 ff-only 反复失败时，修复是「fetch → merge main → 跑门禁 → push → 立刻重试 ff」紧凑循环，把竞态窗口压到秒级；ff 失败不删 worktree，commit 始终安全在分支 ref 上。
- 2026-09-05：当接手任务发现验收脚本在 main 上本来就红（别的波次合并了新裁决没跑该脚本）时，修复是先在 main 复现确认存量、再按「断言语义不弱化」适配新语义并把适配写进提交正文，避免误判为自己改坏。
- 2026-09-05 G4 重复派单：任务书对 worktree 的预设（「他人遗留零提交空 worktree」）必须实证再动手——mtime 新鲜度 + reflog + 存活进程三查在 90 秒内证伪预设（group.rs 实时编辑 + group_contract 编译指纹 + 90 秒前的 git reset），立即停手让位避免双写事故；归属不明先 session_link 逐会话问询核销。
- 2026-09-05 G4 接管：git status 陈旧 stat 缓存会漏报整个脏文件集（33 文件 796 行漂移首次 status 只见 1 个 untracked，merge 写索引后才显形）；接管 worktree 先 git diff HEAD --stat 强制重哈希再信状态。
- 2026-09-05 G4 审计：判定大 diff 是否纯 fmt——git show <c> -- <paths> 删除/新增两侧行去空白后 sort|uniq -c 比对 token 多集，逐对可配对即零语义（rustfmt 换行/尾逗号/match 臂加括号会让 -w 的 --stat 仍有行数差，别被吓住）。
- 2026-09-05 G4 收尾：重复派单撞车时「以 main 合入状态为准」——授权接管的瞬间执行者可能恰好收尾完（本次 rebase 时 fmt/clippy 修复笔被 git 自动丢弃 "patch contents already upstream"），rebase 的自动去重就是最干净的冲突裁决。
