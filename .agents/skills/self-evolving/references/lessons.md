# Lessons

<!-- 一条经验一行。格式：当 X 发生时，修复是 Y。skill 没提前警告我。 -->

_none yet — be the first._
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
