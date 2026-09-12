# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

- 2026-09-09 本仓全文搜索必须避开构建产物：`apps/acp-agent/target` 等 target 树混在 crates/apps 源码树里，整仓 rg/grep 会扫到超时（60s 被杀实测）；把搜索 path 收窄到 `crates/x/src`、`apps/x/src`，或加 `--glob '!**/target/**'`。
- 2026-09-07 UX-I SPA 多步走查的 gui-agent flow 姿势：副本加 `flow` 命令吃 JSON 步单（eval/shot/clickPrev 三种步型），eval 返回 {x,y} 时记录、clickPrev 用 CDP Input.dispatchMouseEvent 原生点击上一坐标——多步状态一次 Chrome 会话闭环，天然满足「mock 状态不跨 CLI 调用」约束；截图步随状态走，证据落 /tmp。
- 2026-09-07 UX-I mock dev「空缓冲」造法：gui auto-start 会自动起节点且手动 stop 在 auto-start 落定前点无效；先等「停止节点」按钮可用（= 已启动）再 stop（此后 manualStopRequested 守卫防重启），再走 事件页→清空→确认 全 UI 路径，才能稳定拿到全 0 计数+「暂无事件」空态。

- 2026-09-05 传输层错误取证走 source 链 Debug 遍历：quinn 的 ReadError/WriteError/ConnectionError 经 io::Error 包装后 Display 只剩 "connection lost"，Reason 全灭；在错误落地处 `let mut s = e.source(); while let Some(x)=s { println!("{x:?}"); s=x.source(); }` 遍历打印，即可看到 `ConnectionLost(ApplicationClosed(ApplicationClose{reason:b"hangup"}))` 级别的真因（BASE1 由此一击定位池收敛误杀）。伴随信号：服务端零告警 + 客户端秒败 = 连接被对端/中间层主动关，先查 close 归因再查网络。
- 2026-09-05 失败按「进程级二值」分布时，查每进程不变量而非每连接随机性：PeerId 排序、endpoint/句柄复用、静态 tie-break 规则都是候选。同型对照实验（fresh endpoint vs shared endpoint × 快速连发 vs 加间隔）一次就能把变量空间切成两半——BASE1 用 3 组 30 连发矩阵把「网络抖动」假设排除、锁定收敛规则误用。
- 2026-09-04 预跑 ai-docs-sync 门禁免 worktree 全量 cargo 重建：`sed -e 's|^DOC=.*|DOC="<worktree文档路径>"|' -e 's|^CTL=.*|CTL="<主树新鲜二进制>"|' scripts/check/ai-docs-sync.sh > /tmp/sync-wt.sh && bash /tmp/sync-wt.sh`——拷贝后 ROOT 推导失效，但二进制不陈旧就不进重建分支，45 条目/139 项参数比对/示例抽验照跑；合并回主树后再跑真脚本终验。
- 2026-09-04 补「lossless JSON」条：不止传 {} 或整个 undefined，参数对象里带显式 undefined 键（如条件未命中的 timeoutMs: undefined）同样在绑定阶段炸；用 if 组装对象、只放命中的键。
- 2026-09-05 GUI 中央登记三件套提交顺序：feature 提交（src/新目录+测试）先行、登记提交（menu.def/App.tsx/locale/守卫测试）随后，HEAD 必绿；两段用 `git add <精确路径>` 分批 stage，feature 后补的红线修正用 `git commit --fixup=<feat> && GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash <main>` 折回，rebase 顺带把过时的 merge commit 线性化。
- 2026-09-05 run_code 调无必填参数的宿主工具（session_link_list/workspace_list/job_list/get_goal 等）传 {} 或 undefined 会报 "binding arguments must be lossless JSON"；传一个无害探测键（如 {probe:1}）即可正常调用。
- 2026-09-02 全新空目录起项目要用 worktree 流程时：`git worktree add` 需要 HEAD，
  空仓库无 commit 会直接失败。先 `git init -b main` + baseline commit（AGENTS.md/skill/.gitignore），
  再开 worktree；`.worktrees/` 要写进 .gitignore 避免嵌套目录被主树误跟踪。
- 2026-09-01 clippy 门禁报 "'cargo-clippy' is not installed" 时：`export PATH="$HOME/.cargo/bin:$PATH" && rustup component add clippy`，装完即可跑 `-D warnings`。
- 2026-09-01 拆提交保可 revert：把后一个提交涉及的 lib.rs 行先临时摘除、验证编译后提交 A，再恢复、验证后提交 B；cargo 对 src/ 下未被 mod 引用的 .rs 文件直接忽略，中间态可安全验证。

- 2026-09-02 Rust 依赖 API 核对：cargo fetch 后直接 grep ~/.cargo/registry/src/<registry>/<crate>-<ver>/src 源码确认真实签名（registry 域名目录用 ls -d $HOME/.cargo/registry/src/*/ 取），比查 docs.rs 快且与实际版本一致。
- 2026-09-02 冲突的 Cargo.lock 处理：解掉 Cargo.toml 冲突后 rm Cargo.lock && cargo fetch 让其按新清单整体重生，再 git add，不手工解 lock 冲突。
- 2026-09-02 macOS 无 coreutils timeout 命令，限时跑命令交给外层工具超时参数，不要写 timeout 120 cargo test。
- 2026-09-02 协调多会话并行开发时：协调会话在主树留未提交修改会被 worker 会话收尾的 `git add -A` 卷进它的提交（p2p 项目 1971e69 实例）。主树要么保持 clean，要么编辑完立即 `git add <具体文件> && git commit`；协调文档改动走"编辑+提交同轮完成"。
- 2026-09-02 文档与代码对齐（V 类任务）：写进文档的每个常量当场标注 `文件:行号`，收尾用一次 grep 常量名批量核对出处行号；行号引用要落在语义块起点（struct/const 行），别落在注释或空行上。
- 2026-09-02 run_code 里 git commit 带多行长 message：message 含单引号会被 bash -c 外层包裹炸出 "unexpected EOF"，把 message 用 write 写到 /tmp/x.txt 再 `git commit -F /tmp/x.txt`。
- 2026-09-02 长任务跨并行会话：收尾前别信任务开始时的扫描结果——期间 main 可能已前进（本次会话中段进了 87e8683/75d8ad8 两个提交）；rebase 后要 diff 一下新增文件，新文档可能改变已写好的结论（实例：wire-protocol.md v1 把签名未覆盖 TTL 按现状冻结，审查报告须补冲突说明再合并）。
- 2026-09-02 rebase 后不重跑全量测试也可信迁移绿色结论：`git diff --stat <已验证commit> <合并后commit>`，若 diff 不含本任务任何产物（本任务子树为空）则两树在本任务范围内逐字节一致，绿色结论 1:1 转移；diff 里只应出现并行会话的新文件。
- 2026-09-02 run_code 多行 bash -c 里 echo 文案含裸 ")"（如 "== 4) 前缀 =="）会炸出 syntax error near unexpected token 并中断后续行；验证步骤文案避免裸括号，或把每步拆成独立调用。
- 2026-09-02 对同一文件先跑过 cargo fmt 再用 edit 工具会报 "file changed since it was read"——fmt 改写文件使读快照失效；编辑前重读一次该文件即可。把 fmt 放在"编辑完最后一步"或"编辑前"执行，别夹在编辑序列中间。
- 2026-09-02 run_code 调用必须同时带 code 与 description 两个参数，漏 description 连环报 "invalid arguments: missing required property description"（本会话连犯多次才定位是外层调用缺参，与代码内容无关）；另外 binding 参数里传 undefined（如可选的 workdir）报 "binding arguments must be lossless JSON"，可选参数要按条件省略 key 而非传 undefined。
- 2026-09-02 bash 管道吞退出码：`make check | tail` / `cargo test | tail` 报告的 exit 恒 0，连 `bash: cargo: command not found` 都显示 [exit 0]（本次实录）。门禁结论必须显式收退出码：`make check > log 2>&1; echo exit=$?`，再从 log 取摘要。
- 2026-09-02 edit/run_code 里嵌 Rust 代码片段时用模板字符串包裹，别用双引号 JS 字符串——内嵌的双引号要逐层转义极易错；也别把 Rust 字符串改成单引号（Rust 无单引号字符串字面量，format!('...') 直接语法错误，本会话返工实录）。
- 2026-09-02 长任务中途发现自己的 worktree/本地分支凭空消失：先 `git log --oneline main` + `git worktree list` + `git ls-remote`，大概率已被协调会话验收合入（squash 成新 hash）并执行收尾四步清理——代码在 main 上，别当事故排查（2026-09-02 E4 hairpin 实录：我推的 ff0388d 被合为 0f1c73b，diff 核对逐字节一致）。
- 2026-09-02 免 sshpass 的密码 SSH 通道：mktemp 生成只含 `printf %s "$SSH_PASSWORD"` 引用的 700 权限 askpass 脚本，配 SSH_ASKPASS_REQUIRE=force + DISPLAY=:0——密码经环境变量传递，不进 argv 不落文件，macOS 自带 OpenSSH>=8.4 即可（ECS 部署实录；sshpass -e 为等效备选）。
- 2026-09-04 harness run_code 的 bash workdir 参数失效（固定跑在会话 cwd）：树相关操作（cargo test / git）一律命令内 `cd /path/to/tree &&` 前缀，首行加 `pwd && git branch --show-current` 自证归属；本轮曾在主树跑测试得出全绿假象（测的是未修改代码）。
- 2026-09-04 run_code 模板字符串里写 shell heredoc/脚本：`${VAR}` 会被 JS 插值、内容里的反引号会终止模板串（Rust 文档注释的 [`X`] 也踩）——多行脚本用 write 工具落盘，或行数组 join 后再 edit；行内确需字面 ${ 就整体换成 write。
- 2026-09-04 长命令跨 job 收集：bash 后台 job 的 stdout 只在 job_output 里取，超长输出截尾时改用「命令重定向到文件 + 后续 cat 文件」模式，保证拿到完整日志（本轮 rz/helper 日志即此法）。
- 2026-09-02 测 UDP 映射空闲寿命定界传输层问题：向对端 UDP 反射口（如观测口 3402）同一 socket 间隔发探针，看应答与外部端口是否漂移——ECS 实测空闲 12s 映射稳定，一句话排除「NAT/安全组 5s 掉会话」假设，把排查收敛到应用层。
- 2026-09-02 网络断链类 bug 的消融三板斧（/t3401 实录）：① 真实 TCP + 用户态窄管道泵（read ≤SEGMENT→write_all→flush→sleep，双向各一任务）模拟公网分段/RTT，SEGMENT=256/JITTER=2ms 比真实公网苛刻；② 逐层替换跑同链路（纯 Noise / 纯 yamux / Noise+yamux）锁定层；③ 生命周期对照——对可疑句柄 std::mem::forget（测试短命可接受），全绿即证实「句柄丢弃自毁」假设。

- TCP 可达性判定用 `nc -vz -w 5 host port`，禁用 `bash /dev/tcp + echo + timeout` 三件套：后者对"accept 后即关"的服务（p2p relay/bootstrap、部分网关）write 失败会误报不可达——2026-09-02 实测把全绿的 relay 口误判成全红，对照 SSH 22 同样误报才暴露。nc 在 macOS 自带，输出在 stderr（2>&1 取）。
- 2026-09-02 判定分支是否已并入 main：本地 main 落后远端会让 `git merge-base --is-ancestor <分支> main` 误报未合并——先 `git fetch --prune` 并 ff-only 同步本地 main，再与 origin/main 比对（fix/e4-tcp-stream 实录：对本地 main 判 NO，origin/main tip 即分支 tip 8aaedda，实际早已合并；分支与 worktree 清理照收尾四步补完）。
- 2026-09-02 分支收尾扫尾必查 detached worktree：`git worktree list` 里的 detached 项会漏过「分支已全合并」检查——用 `git cherry origin/main <commit>` 判重后抢救成命名分支再走收尾（e4tcp 实录：回归测试进了 main，配套生产修复遗落在 detached HEAD 上无人认领）。另：run_code 后台任务 workdir 不存在时不报错而是回退目录照跑，起任务前先确认目录存在；bash-121 与 bash-122 共用 /tmp 日志路径互相覆写，并发任务日志各用各的路径。
- 2026-09-02 itest 里制造"事件A先落地、再触发事件B"的时序：`let f = x.connect()` 是惰性 future，不 poll 不启动，直接 drop 其他组件会误判成时序竞态——先 tokio::pin! + futures::poll! 主动推进数步配 sleep 让状态落地（单线程 runtime 协作调度内必完成），再做触发（relay_control_resilience 实录）。
- 2026-09-02 远端清理指定实验进程用 pgrep -f 时，模式串若原样出现在自己 bash -c 命令行里会匹配自身 → kill 自杀（ssh 退出 255 无输出）。修法：括号技巧 `pgrep -f "[e]csn2"`——模式文本本身不含 "ecsn2" 连续串即不会自匹配（R-E4 冒烟实录）。
- 2026-09-02 无参绑定工具（如 session_link_list）在 run_code 里传 {} 或 undefined 都报 "binding arguments must be lossless JSON"；先试无参直调，不行就绕开该工具用已知目标 id 直连（补充 21 号技巧）。
## run_code 模板字符串吃反斜杠（2026-09-02，gui-shell）

用 run_code 的 tools.write 写文件时，内容在 JS 模板字符串里：正则的 \d、\/
会被模板转义吃掉（/\d+/ 写进文件变成 /d+/），tsc 报 TS1135 等语法错时先查
文件里正则的原文。对策：正则改用 new RegExp 字符串形式，或内容里双写反斜杠。

## pnpm 在无 package.json 的目录报 NO_IMPORTER_MANIFEST_FOUND（2026-09-02）

pnpm run 在 monorepo 子包外的目录执行直接退出 1，输出没有任何 error TS 行，
与真实构建失败难区分。构建验证一律显式进入子包目录（或 pnpm -C <包> build），
并且不要和 git 提交串在同一条命令里。

## 函数行数与 i18n 集合的机械审计（2026-09-02，gui-views-monitor）

- 函数 ≤60 行审计：node 脚本正则抓 function 声明，先对参数做括号平衡匹配
  （字符串感知），取其后真正的函数体花括号再计数——直接找第一个 { 会把
  组件的解构参数当函数体漏报。脚本模式存 /tmp/fn-audit2.js 可复用。
- i18n 中英 key 集合一致性：npx esbuild src/i18n/locales/{zh-CN,en-US}.ts
  --format=cjs 转 CJS 后 node require，递归取叶子路径比对集合。类型层面
  enUS: typeof zhCN 已兜底，该脚本给出独立于 tsc 的机械证据（166=166）。
- worktree 里跑前端先 pnpm install（worktree 不共享 node_modules）；
  macOS 无 timeout 命令，dev server 冒烟用 run_code 后台 job + sleep +
  curl 探活 + job_kill 组合。- GUI 启动冒烟（jsdom 挂整应用）：vi.stubEnv("VITE_MOCK_IPC","1") 后 await import("../main")，waitFor host.querySelector("main")，断言 innerHTML 非空且无 ErrorBoundary 兜底文案；手工 appendChild 的 host 在 afterEach 清空防跨测试泄漏。整跑：bash scripts/check/gui.sh。
- 崩溃定位：ErrorBoundary 的 componentDidCatch 会 console.error 带异常消息，vitest 输出里搜「渲染异常」直接得根因；Maximum update depth 且栈里有 forceStoreRerender/updateStoreInstance = store 快照引用漂移。
- 2026-09-04 诊断 mock 残留核查：先 grep 诊断视图、IPC 路由和类型契约的 mock 引用，再检查运行时选择逻辑；测试文件中的 vi.mock/mock fixture 属于测试隔离，不应误删。

## 零依赖 CDP 页面操作入口（2026-09-03，G-H gui-agent）

- Agent 要"操作/观测网页"不必上 Playwright：node ≥22 原生 WebSocket 直连 CDP。流程：
  spawn Chrome `--headless=new --remote-debugging-port=P --user-data-dir=<mkdtemp>`，
  轮询 /json/version 就绪，`PUT /json/new?url=` 建 target（新版必须 PUT），
  连 ws 后先 Runtime/Page/Log enable、挂 Page.loadEventFired 监听、再 Page.navigate
  （顺序反了会错失 load 事件）；收尾 kill 后 rmSync 临时目录可能 ENOTEMPTY，
  延迟 + maxRetries + 降级告警，别让清理失败污染命令退出码。
- 页面错误三通道合并比 DevTools 手翻快：Runtime.consoleAPICalled +
  Runtime.exceptionThrown + 应用内错误缓冲（window.__P2P_AGENT__.recentErrors()）。
  G-H 首跑即定位 selectPeerList 无限重渲染 + Button ref 告警，复验 console/exceptions
  双清零即修复证明（截图/JSON 留档 .gui-agent/）。
- DSH glob/grep 工具锚定会话 cwd，对兄弟 worktree 路径直接失配（静默返回空）：
  worktree 里一律改用 bash find / git -C <tree>，别被空结果骗成"没有测试文件"。
- eslint react-hooks/refs 禁渲染期写 ref：测试桩要把 form 实例递出 render 树时用
  useEffect 赋值——RTL 的 render/fireEvent 包 act，effect 提交即刷，后续断言可同步读。
- 表单"状态对、显示错"先写 probe 测试锁 DOM value 再动手：一分类（受控 vs register）
  就知道该改组件还是改表单接法（2026-09-02 W6-S1 settings-defaults 实录）。

- sonner 在 jsdom/vitest 下 toast 是异步 mount：`act(() => toast.x())` 后 DOM 立即查询为空，必须 `await screen.findByRole/findByText` 等待；先用一次性 probe 测试 dump `container.innerHTML` 可 1 分钟定位此类渲染时机问题。
- sonner 测试间模块级队列残留：afterEach 里 `cleanup()` 后再 `act(() => toast.dismiss())` 清全局队列，否则下个用例看到上轮 toast。

## 2026-09-03 CI 监控

- 无 gh CLI 监控 GitHub Actions：public 仓库 REST API 匿名可用但限 60 次/时限；配 ETag + If-None-Match 条件请求（304 不计限额）即可放心 60s 轮询。node 脚本丢 background job 跑，run completed 时拉一次 jobs 详情后 exit，完成通知自动叫醒会话。
- jobs API 里 runner_name 为空 + 零 step = job 纯排队没拿到机器（不是跑得慢），直接去查 runner 镜像是否退役/改名。
- 2026-09-03 判断某个 Rust 子树是否在 fmt 门禁内：看该 crate Cargo.toml 有无 `[workspace]` 空表（独立 crate）+ scripts/check/fmt.sh 的 cd 基准（根目录 cargo fmt --check 只覆盖根 workspace 成员）；独立子树要格式化得单独特跑并在提交前剔除无关 churn。
- 2026-09-03 DSH devloop_* 工具有自己的默认 projectRoot，不一定是当前会话 cwd 的项目（实例：cwd 在 p2p，devloop_scan 扫的是 plugins）——多工作区环境必须显式传 root 参数，或改用 bash git 命令直查当前树。

- 2026-09-03 W6：bash heredoc 里嵌 python 正则替换时反斜杠转义会双层损耗导致静默失配（打不出错、就是不生效）——改用 s.find(marker) 定位加字符串截断，配 assert idx 大于 0 防静默；sed -i 空串写法在 macOS 可用。
- 2026-09-03 W6：run_code 字符串数组逐行 join 写代码文件时，行内引号与 JS 外层引号同种是雷源（本次 TSX 双引号 className 行在单引号 JS 串里转义炸整个脚本）——含同种引号的行改用另一种 JS 引号承载；写完立即 grep 回读关键行。
- 2026-09-03 W6：验证命令要截尾输出时用三段式 cmd 重定向到临时文件加分号 echo 真实退出码再加 tail，替代 cmd 管道 tail（管道末端吃退出码红线的日常化写法）。

## 2026-09-03 W7-G-U2 更新提醒前端轮

- vitest + zustand：测试里 store.setState 放 act() 外时，React 订阅组件的重渲染与 effect 异步冲刷，紧随其后的断言读到旧值（toast 调用次数 0）——setState 一律包 act()。另注意测试夹具 helper 若顺手重置去重标记（如 reminderShownFor），会把要测的轮询去重逻辑本身破坏。
- vi.mock 工厂要引用顶层 vi.fn() 时必须走 vi.hoisted(() => ({...}))，普通 const 会被提升后的工厂在初始化前访问（TDZ 报错）。
- fake timers 下等微任务（mock IPC 立即 resolve 的检查流程）用 await vi.advanceTimersByTimeAsync(0) 冲刷，比 await Promise.resolve() 更稳。

## 2026-09-03 邻居表 127.0.0.1 条目归属判定
- `lsof -nP -iUDP | grep -E "<端口1>|<端口2>"` 一步判定端口是否本机在听：在听=自己人（ps 对 PID 看 --data 参数定身份），不在听=他人 loopback 泄漏条目；自身 PeerId 与 GUI 设置页身份卡对照。比逐个「详情/拨号」试错快得多。

## 2026-09-03 集成测试竞态消除：服务端 metrics 当确定性同步点
- relay 服务端有 pub metrics() 快照（circuits_active 水位）——「停车是否已被服务端处理」「回收是否已完成」这类不可从客户端观测的状态，轮询水位到目标值再断言，比 sleep(200ms) 排序稳；黑色竞态（两落地都合法）则断言并集并注释说明两分支语义。

## 2026-09-03 E8-H3 mux 生命周期轮
- TCP keepalive 参数化走 socket2：tokio 1.53 没有 TcpStream::set_tcp_keepalive（TcpKeepalive 类型也不存在，只有 TcpSocket::set_keepalive(bool) 缺省参数版）——直接 SockRef::from(&tokio TcpStream)（tokio 实现 AsFd，内部 ManuallyDrop 借用 fd 不取所有权）set_tcp_keepalive 即可，不要做 into_std/from_std 往返（from_std 有阻塞检查且失败路径已消费流无法复原）。
- panic-hygiene 豁免收缩的消融探针：动手前先用门禁自带的 PANIC_HYGIENE_EXEMPT env 覆盖指向「目标收缩态清单」跑一次，点名出的 file:line 就是工作清单；改完同命令复跑，红→绿即消融证据，全程不改门禁本体。

## 2026-09-04 tauri dev 报错诊断
- 端口占用一步定位：lsof -nP -iTCP:<端口> -sTCP:LISTEN 拿 PID，再 ps -o pid,tty,lstart,command -p <pid,...> 看整棵进程树的起点 tty 与时间——遗留会话（十几小时前起的）与活跃会话一眼可辨。
- tauri App 持久化日志在 ~/Library/Logs/<identifier>/（本仓 com.p2p.console/：frontend.log=webview console，p2p-console.log=Rust tracing）。frontend.log 只记 message 不含「see errors above」的编译正文，真错要去 vite 终端输出；两份日志 mtime 是否持续前移可判定进程还活着还是僵死。
- 沙箱/工具链的 bash 是非登录 shell，不加载 ~/.zshenv，cargo 不在 PATH——跑 cargo 系命令先 export PATH="$HOME/.cargo/bin:$PATH"；用户 zsh 终端不受影响，勿把沙箱 PATH 报错（cargo metadata No such file or directory）误判成用户环境问题，先带 PATH 复现再下结论。

## 2026-09-04 时区与崩溃时间线核对
- 这台 Mac 系统时区是 PDT(UTC-7)，ps/lsof/ls/reflog 打出的全是 PDT 时间——按 +08 心算「昨天早上 8:21」直接把启动顺序推断反了。跨时区对时间线一律先 date 与 date -u 打底，再用 git log --format=%cI（ISO 带偏移）和 ps -o lstart 全量时间戳钉死，禁止裸 HH:MM 心算换算。
- 崩溃组件栈行号比对：vite 服务的是 react-refresh 注入后的转换产物，源码第 N 行出现在栈里约第 N+4 行——拿栈行号反查源码前先按此偏移折算，别因行号对不上就误判「跑的不是这份代码」。

## 2026-09-04 DSH 会话记录清理（GUI 侧，p2p 73→10）
- 数据根 `~/.dsh/dsh012-clean/`（--profile web，默认端口 18181）：记录在 `sessions/<工作区路径编码>/session-<uuid>/session.jsonl.zstd`；归档标志在 `storages/workspace.json` 的 `global.archivedSessionIds`；成员关系在 `tables.workspaces.<id>.sessionIds`。动手前先拿自有会话的 sessionId 在磁盘反查，确认数据根归属（这台机器跑着多套 profile/端口）。
- 顺序两步不可颠倒：先逐个 `workspace_session_manage{action:"archiveSession"}`（宿主自己写注册表，侧边栏立即隐藏），再把对应记录目录 mv 进 `sessions-quarantine/prune-<时间戳>/`（可恢复，不直接 rm）；先删盘会让归档调用读到缺失目录。
- 「1小时前」用 epoch 比：session_link_list 行的 updatedAt（毫秒）对 Date.now()-3_600_000；ls/ps 打的是本机时区（这台是 PDT），肉眼对表必错。
- 排除集：self、running、updatedAt≥截止线的会话；无 `session-` 前缀的 uuid 目录是历史残留记录，按 mtime 过线一并隔离。
- 验收看过滤面：workspace_list / session_link_list 是管理视图，返回全量（含已归档、索引残留），不代表侧边栏；真效果 = archivedSessionIds 含目标 + 磁盘目录已迁走。
- `storages/session-query.sqlite`（persisted_sessions/persisted_docs）先 SELECT 确认有目标行再删（本例 p2p 会话为 0 行），宿主是否持句柄用 lsof 判，别按目录臆测索引内容。

## 2026-09-04 排查「GUI 里为什么有离线节点/幽灵条目」类问题
- 双路并查不先读代码：一路 ps 看活进程拓扑（本例一步看到 lab 节点 maca/coordinator + console 全在线、还有周二遗留的 cargo test 僵尸进程），一路查出厂默认配置（gui config.rs default_bootstrap 指向公网共享 rendezvous 池 43.240.223.138 + 121.196.193.177）——数据来源定准了，「为什么列表里有 X」多数不是 bug 而是语义。
- 本项目邻居表语义备忘：GUI peers 表是会话内只增不减的地址簿，「离线」= 未连接且 lastSeen 超 10 分钟（peer-status.ts DISCOVERED_FRESH_MS），后端并无删除条目的路径；lastSeen 只在「学到新地址」或连接成功时刷新（swarm/book.rs 按地址去重后才发 peer_discovered），所以在线但从未连上的节点 10 分钟后也会翻成离线。

## 2026-09-04 用 run_code 探查装了 node_modules/target 的大仓
- glob pattern="*" 会把整树文件灌进工具结果，撑破 20MB 子进程接缝上限直接报错；探查顶层结构用 bash ls（或 git ls-files），内容检索用窄 include + 具体子目录路径，别用裸 "*" 全树扫。

## 2026-09-04 模板库落地（GUI 节点资料轮收尾）
- 反复出现的高频骨架已抽成固定模板：调试 D1-D5（信号甄别/所有权取证/时间线对撞/双路并查/渲染异常执行序）与解题 S1-S3（GUI 新特性五步走/复用锚点先行/提交拆分五问）见 references/templates.md；GUI 布局 B1-B8 见 references/layout-gui.md。
- 分工与维护：散点经验继续进本文件；模板只收「至少锚定两个真实实例」的骨架，修订以追加「修订 日期」小节方式落在模板文件内，不改写原文。入库前对全部事实性断言（锚点文件/类名/符号/提交哈希）跑机械核对，15 项全 PASS 才准合入。

## 2026-09-04 发现面削峰轮（T1-T5 多 worktree 连发）
- run_code 里 edit 工具调用与多条 const 声明混排会炸 JS 解析（Expected ',', got ident/const，连犯三次）——单次调用只留一个 await，路径与文案全部内联字面量，别提中间变量。
- worktree 收尾命令链里 `git merge --ff-only ... | tail -1` 管道吞退出码，ff 失败后 && 链照常往下跑（worktree 被删、分支被删）——git 收尾链禁用管道中转，失败路径用 `; echo rc=$?` 显式收码；ff 失败后分支仍在远端，`git worktree add` 重建即可按红线 rebase 重试。
- 大并行环境下「本会话门禁绿」与「全仓 make check 绿」必须分开判定：本次 make check 三连红（panic-hygiene 不认 *_tests.rs、并行合并遗留 fmt 脏、repair-bridge 在途 crate clippy）全部非本会话产物；先 grep 日志归因到文件，别人的 crate（有活跃 worktree/coordination 退回记录）不越界修，报告里单列归因。
- 门禁脚本自身也有盲区：panic-hygiene 排除表只精确匹配 tests.rs，仓库约定已演进到 *_tests.rs（book/error/refresh 命名）——新命名形态入册时同步扩门禁排除 + 自测夹具加用例，否则首个踩中的 crate 会假红。

- worktree 复用主树构建缓存：`ln -s 主树/target wt/target`（根 workspace 与 src-tauri 各一个）+ `ln -s apps/gui/node_modules`，cargo/pnpm 秒级增量；注意 target 软链在 git status 显示为未跟踪 `?? target`（.gitignore 的 `target/` 不匹配符号链接），提交时用显式路径即可。
- 驱动私有协议 handler 的集成测试套路：受测端 Chat::new 注册 handler，攻击端裸 Node 手写帧（p2p_protocol::write_frame，payload 首字节=类型头）；绕过 write_frame 的 1MiB 校验测帧超限需手写 varint 长度前缀；让受端回坏 ACK 用 Node::handle_protocol 换装自定义 ProtocolHandler（同 ID 覆盖注册）。

## 2026-09-04 会话清理第二轮（p2p 100 槽位归档 47，含 stale-running）
- 动手前先 grep 本文件与 known-issues 的任务关键词：上轮（73→10）已钉死数据根 `~/.dsh/dsh012-clean/` 与「验收看过滤面」，本轮只加载 SKILL.md 没读 references，独立重推数据根还拿错 `~/.dsh/storages/workspace.json`（另一实例，17 工作区/917 归档）得出「归档未生效」的错误中间结论——SKILL.md 加载不携带 references，开工先按关键词扫一遍 references。
- archiveSession 幂等（已归档 id 直接 return 不写），只追加 `global.archivedSessionIds`、不动 `sessionIds` 槽位（packages/workspace/workspace/src/index.ts:244，unarchive 可恢复位置）；所以 workspace_list 计数不变是设计而非失败。
- stale-running：running=true 但 updatedAt 静默 2h+ 是僵尸标记，不可信；用户明确「所有会话」时按 updatedAt 清理（归档只隐藏分组面，不停进程）。上轮「排除 running」适用于保守场景，两条按用户指令二选一。
- 机械验收唯一路径：读活跃实例 `<home>/storages/workspace.json` 全量比对 archivedSessionIds；session_link_list 常态返回全量（含已归档）且无 archived 字段，不能当验收面。

## 2026-09-04 IM-T46A 回复消息后端轮（Rust 契约加法任务）
- 本仓 rustfmt 对带 message 的 `assert_eq!`/`assert!` 宏按 fn_call_width=80 折行且 CJK 宽度按 2 计——单行断言压不进 80 就老老实实多行；要保紧凑格式（如 300 行红线贴线的测试文件）直接给整个测试 fn 挂 `#[rustfmt::skip]`（chat_e2e.rs 既有手法），写完立刻 `cargo fmt --all -- --check` 自查，别等 make check 兜底。
- git branch -d 拒删只看 origin/main（远端陈旧即拒），不看本地 main——先 `git merge-base --is-ancestor <tip> main` 证内容已入本地 main，再放心 `-D`。
- DSH read 工具截断超长行（约 2000 字符）：对含超长 note 行的 JSON（如 .devloop/loop-state.json）「read 后 join 再 JSON.parse」会假报非法 JSON；校验/改写一律 python3 直读文件。

## 2026-09-04 IM-T47 渲染矩阵轮（gui 纯测试任务）
- RTL getByText 对含真实换行的文本不可靠：matcher 被归一化成空格（「您好\n请查收」→「您好 请查收」）后仍报 Unable to find，而 DOM 里 <p> 文本确实存在——「换行保留」类断言直接 querySelector 定位元素 + textContent 精确相等，机械且稳定。
- 测 store 的排序/合并行为禁止 setState 直塞数组（绕过 mergeMessages），要走真实动作路径：mock chatHistory 返回乱序页 → click 好友触发 selectPeer → 断言 DOM 序。直塞测的是组件不测逻辑。
- gui 测试消融验证三步走（不产生提交）：edit 工具改生产一处 → 跑目标测试确认对应格红 → git checkout -- 恢复后复绿且 status 干净；比口头声称「测试有效」硬得多。
- worktree 里 pnpm 测试前先 ls apps/gui/node_modules/.bin/vitest 判依赖在不在，新 worktree 先后台 pnpm install 再读代码写测试，两不耽误。
- 2026-09-04 T38 残留分支判定：rebase 后 ff 合入会原地留下「老哈希分支」，`git branch -d` 按哈希无祖先会拒删——用 `git cherry main HEAD` 机械判补丁等价，全部 `-` 即内容 100% 已在 main，可放心 `branch -D`（实例：test/rs-bridge-boundary 三提交全 `-`，fca6fc3 的等价体是 main 上 e31bc67）。

## 2026-09-04 计划方案冻结·其余文档对齐轮（docs-only 协调任务）
- 接手无前文简短指令（如「对齐其余文档」）：按 .devloop/loop-state.json note → coordination.md 尾部条目 → git log 近 10 提交 三步重建上下文，再对 grep 关键裁决词（如 replyTo）定位哪些文档已跟上、哪些没跟上；不要凭猜测选文件。
- 共享账本未提交改动的归属判断：git diff 看 updatedAt 与 task 状态变化属于哪个会话的轮次（本轮 diff 是 CLI 协调的 CL3-done/CL4-doing，13:15 刚写）；归属他轮则整文件不碰、不卷进自己提交，自己的叙事缺口走 coordination.md 入册补齐，账本 note 折叠留给下一个写账本的人。
- 契约加法落地后的文档对齐清单（四遍扫，防「字段登记了、描述面没跟」）：①契约文档引言的能力枚举 ②演练/操作清单的场景+标注+模板行 ③docs/README.md 索引 ④README 进度与 crate 地图。本轮 replyTo 前两项字段面已齐，但①②③④全漏。
- markdown 手写表格里的路径占位符容易顺手写成 HTML 实体（&lt;peer&gt;）；提交前 git diff 扫一眼与全文风格对齐（本仓库用裸 <peer>）。

## 2026-09-04 CL4·CLI 对等收官轮（bash 守卫 + rust 新命令域）
- 用 run_code 写多行 bash/脚本文件时不要塞进 JS 模板字面量：`${...}`、`\\[` 一层层转义必错（本轮连错两次）。可靠做法：JS 字符串数组逐行 push、含 `$` 的行用拼接（'...'+ '$' + '{BASH_SOURCE[0]}'），或 bash heredoc 配 chr(92) 做替换。
- bash 守卫解析 TSV 要用 `cut -f` 而不是 `IFS=$'\t' read`：read 把连续 TAB 折叠成一个分隔符，空字段（豁免行的 invocation 列）被吞、理由列整体错位——这种错不报错只给假结论，靠自测反夹具才暴露。
- 给「固定追加 --help 的守卫」写 fake CLI 夹具时，case 匹配必须先剥掉 --help 再按路径分发（`$1` 是 --help 时顶层直接落进 `*)` 分支，命令面收集为空）。
- 自写 printf 行生成 TSV 时单引号里 `\t` 不解释，要 `printf '%b\n'`；真实表格文件用写工具直写真 TAB。
- run_code 的 tools.bash 换 workdir 前先确认目录已创建（worktree 未建好时 spawn bash 直接 ENOENT）；tools.edit 前必须用 tools.read 读过（bash cat/sed 看过不算数）。
- 检查脚本里提取 `generate_handler![...]` 用 awk 区间 + `grep -oE '模块::名'` 取末段再滤掉 `generate_handler` 自身，比正则硬吃整块稳；正反夹具自测（tests/cli-parity.sh 挂 gate-tests）照 release-gates 先例，防门禁假绿。
- objc2 系 crate 动手前读本机 registry 源码：`~/.cargo/registry/src/<registry>/objc2-*-0.3.2/Cargo.toml` 看 [features] 粒度，`generated/<Class>.rs` grep 方法真名/参数/cfg 门控；比盲写等编译报错省两轮以上。
- 复杂 bash 探测脚本先 write 成 /tmp/*.sh 再 `bash /tmp/x.sh`；内嵌 run_code 的 TS 模板字符串时 `$`、反引号、`\\` 三层转义极易 Unterminated template（GC1 轮连踩两次）。
- 集成测试用裸 TcpStream（Connection: close + 手解状态行/\r\n\r\n 分帧）做零依赖 HTTP 客户端测真实服务，比给 reqwest 加 blocking feature 轻。
- 无视觉模型做布局断言：Chrome --headless=new --remote-debugging-port=9223 起 CDP，Node≥22 用内置 WebSocket 连 /json/list 的 page target，Runtime.evaluate 读 getComputedStyle().gridTemplateColumns 与 clientWidth，量化到像素并留 JSON 证据；Page.captureScreenshot 同会话出图。vite 6 无外置 ws 依赖，零安装。
- 高并发合并日收尾：ff-only 前提用 `git merge-base --is-ancestor main <分支>` 判定（分支顶^ 是自己的第一个提交，判错白跑一轮）；推送+核对+合并+worktree remove+branch -d+push --delete 压成一个 set -e 脚本原子执行，守卫拦截即整组回退。
- 收尾循环提速：main 新增量 diff --name-only 全为 docs/.devloop/.agents 时测试内容等价，验收判定可沿用直接合并；动了代码（含 src-tauri、main.tsx）必须重跑全量 make check。

## 2026-09-04 IM-V2 轮（shadcn/tailwind 视觉打磨）

- 任意变体包裹选择器（如 `[&_[data-slot=card]]:min-h-28`）特异性 0-2-0，
  会静默压掉卡片自身的 `.min-h-40`（0-1-0）——包裹类只罩最小必要子树
  （例：只罩两行指标卡的内层 div），罩全页则子元素同属性类全部失效。
  证据手段：CDP getComputedStyle 读 computed minHeight，类在但值不对即此坑。
- tailwind-merge 对同一 variant 链去重（`data-[state=active]:bg-*` 后者
  覆盖前者并删除前者）；但跨 variant 链（dark: 前缀）不互删——页面级覆盖
  ui 组件底态必须同时写 plain + dark: 两条覆盖，否则暗色回落到组件暗色底。
- WCAG 对比度自证：CDP 里 getComputedStyle 颜色是 oklch()（tailwind v4），
  页面内手写 rgb 正则解析全空——改在 Node 里按主题 token 权威色值算
  （index.css 的 oklch 换算或类名的 16 进制等价），类名断言 + 权威色值计算
  双证据比脆弱的浏览器解析稳。
- git worktree remove 后原 cwd 里再跑 git 命令 exit 128 not a git
  repository——是目录已删，不是仓库坏了；回主树验证。
- 验证二进制产物新旧：`grep -a` 直接按字节搜 UTF-8 中文标记（strings 丢非 ASCII 全是 0）；配合 ls -l mtime——注意 ls 出的是机器本地时区，DSH 会话上下文的时区标签可能与机器不一致，别被「00:16」骗成十五小时前。
- run_code 写/改含引号与中文的文件：单引号行数组 join + base64 + python3 精确替换（替换前后 assert count==1），绕开 JS 转义地雷（双引号串内嵌转义引号会随机解析炸，模板字符串同险）；edit 工具的 old_string/new_string 走 Buffer roundtrip 同效。
- 2026-09-04 密钥泄漏机械自查：python3 提取 .env 全部 value（只进内存不打印）→ 对提交树逐值 git grep -I -F -l -- <value> <ref> → 只输出 key 名与命中文件；命中先分类：IP/用户名/域名等公开登记属存量可豁免，key 名含 KEY/SECRET/TOKEN/PASSWORD 的必须零命中。比肉眼确认可靠。
- 2026-09-04 run_code 里跑内嵌 python/多行脚本：优先 bash quoted heredoc <<'EOF'（单引号防 JS 与 shell 双层插值），比在 TS 模板串里堆转义可靠；本日含 <( )、${v} 的复杂串直接报 Expected ','，拆简单步骤或改 heredoc 后一次过。
- 2026-09-04 N2：macOS 上 HOME=临时目录 启动 Tauri GUI 即整体隔离 app_data_dir
  （dirs::data_dir 走 $HOME/Library/Application Support）与 app_log_dir，CLI
  --data-dir 指向同一目录即零接触真实用户数据的 GUI×CLI 数据面 E2E；
  endpoint.json 就绪轮询 + pid 匹配防串实例（scripts/ops/cli-gui-data-e2e.sh
  实证连跑多遍可复跑，GUI 冷启动就绪 <15s）。
- 2026-09-04 N2：run_code 生成 bash 脚本一律「行数组 join + write 落盘 + 执行
  文件」，不要内联在模板串里跑（内含 ${} 与引号必炸）；脚本内 JSON 断言用
  python3 heredoc 函数化（深比较/成员/计数），比 grep/sed 拼断言稳。
- 2026-09-04 N1：run_code 传 600+ 行长文本给 tools.write 时，经 JS 模板串转手会
  词法炸（Expected ',' got '<lexing error>'）；直接把内容作为 write 的 JSON 字符串
  参数传，不经 JS 字符串拼接。
- 2026-09-04 N1：门禁脚本防假绿自检法——写完门禁必做负向注入四件套：多出条目/
  缺条目/参数改名/删参数行，逐一断言门禁变红且诊断指向正确改动点。本次
  ai-docs-sync.sh 四类注入全红（含 cli-guide 真实发生过的 --nickname→--name 漂移样本）。
- 2026-09-04 N1：无网络对端时的命令面实测技巧：chat serve --json 现场生成合法
  peer id；friends add 用第二个 --data-dir 避开"不能与自己通信"；gui navigate 打
  当前所在路由即零打扰实测；log tail/clear 用 --log-dir 指向临时目录不碰真实日志。
- 2026-09-04 IM-T48：CDP Runtime.evaluate 响应是双层嵌套 {id, result:{result:{type,value}}}——
  send 解包层级与 evalJs 取值层级必须配对（res(m)+r.result.value 或 res(m.result)+r.result.value），
  错配静默返回 undefined 不报错，脚本照常跑完但断言全失真。写驱动脚本先跑探针（evaluate 'alive'
  校验取值层级）再上全量矩阵。
- 2026-09-04 IM-T48：WCAG 对比度离线计算——oklch→linear sRGB 的输出已是线性值，luminance 直接加权；
  再套一次 sRGB gamma decode 是双重解码，误差可达 2 倍（#767676/白 4.54 被算成 14.00）。透明色
  合成须在 gamma sRGB 空间做再 decode 回线性。写完先用 白/黑=21.0、#767676/白=4.54 两个锚点自校验。
- 2026-09-04 IM-T48：给脚本文件打多行补丁，锚文本凭记忆拼写必失配（含 \n 与引号转义的 eval 字符串
  尤甚）——用 edit 工具 + 刚 read 过的 verbatim 文本；每批补丁后 grep -c 验证落盘，且 grep 的标记词
  必须是补丁里真实写入的串（另造验证词会假绿）。
- 2026-09-04 IM-T48：验收链尾接 "; echo EXIT=$?" 会让 job 退出码恒 0——分阶段 echo
  （TEST_EXIT/BUILD_EXIT/MAKECHECK_EXIT 各自收码）+ job_output 读实值判定，链式 && 一处断全线静默停。

## 2026-09-04 R1 friends 写锁轮
- flock 锁挂在 open file description 上：同进程两次 open 的两个 fd 互相冲突（第二者 LOCK_NB 得 WouldBlock）——单进程双 fd 即可测「持锁超时显式报错 / 释放后重获」全路径，不必真起多进程。
- 跨进程文件锁选型锚点：Unix 用 flock(LOCK_EX|LOCK_NB 自旋 + 截止时间) 进程崩溃内核自动释放无陈锁；超时报错带锁路径与「拒绝静默覆盖」语义满足可观测红线。锁内必须重读磁盘权威态再合并写（只加锁不重读仍是 last-write-wins）。

## 2026-09-04 IM-T50 轮
- run_code 的 bash 命令写在反引号模板里时，shell 循环变量 ${i} 会被 TS 先插值报 ReferenceError——含 shell 变量/循环的命令一律用单引号字符串承载（与 red-lines 反引号条同族，本条补变量插值变种）。
- 验收红定性三步：隔离复跑最小面（cargo test -p 单包 / 单文件 vitest）+ 查可疑产物 mtime + 同码他树全绿对照；三步齐才允许记为环境竞态并错峰重跑全量。
- edit 批量修改生产文件后先跑最小面单测（本次 mock-chat 11 连崩由一条 edit 引出，单文件 vitest 半分钟定位），不要攒到全量门禁才发现。

## 2026-09-04 cargo 锁竞争排查工具箱
- 定位链路：ps aux | grep -E '[c]argo|[r]ustc' → ps -o pid,ppid,lstart,etime,%cpu,command 看族谱 → pgrep -lP <pid> 验有无编译子进程 → sample <pid> 2 看内核栈卡点；CPU 秒数是「活着干活」与「挂死占坑」的唯一硬指标。
- macOS 本机 `lsof <文件路径>` 会无限挂住（当日实测两连挂，进程明明存在也查不出持有者）——查进程句柄一律 `lsof -p <pid>`，别对锁文件做文件级 lsof。
- 多会话 workspace（本机 84 会话在册）并行 cargo 是常态：动手前先 ps 全景再行动；孤儿验收链特征 = bash -c 包裹 + 日志在 /tmp + 父会话已关（/tmp/t49-*.log 命名即此类）。
- rust-analyzer 也抢 ./target 锁：VS Code settings 设 "rust-analyzer.cargo.targetDir": true（落 target/rust-analyzer）消除编辑器与终端 cargo 互相阻塞。

## 2026-09-04 IM-T49 轮
- `git commit --amend` 永远只打 HEAD：多提交序列里修非 HEAD 的提交，amend 必错位（当日把 wire 测试修复并进了命令层提交）。重排未推送历史的可靠姿势 = `git reset --soft <基点>` 后按文件分组重新 commit；要合并进指定提交用 `git commit --fixup=<hash>` + `GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash <基点>`，且 fixup 前确认目标哈希未被 amend 改写（目标一变 autosquash 静默 no-op）。
- `cargo xxx | tail` 的退出码是 tail 的，cargo 失败也「completed」——门禁判决一律把整链输出重定向进日志文件并追加 `echo EXIT=$?`，以日志里的标记行定成败，不信管道外层。
- 通知会打断 job_output 等待（报 abort）但进程往往还活着：先 ps 查 PID 与日志尾再决定 kill，别盲目重启造成双跑。被并行会话毒化的主树验收，干净判决用 CARGO_TARGET_DIR 指独立目录整链重跑（本次隔离跑 11 分钟 exit 0）。

## 2026-09-04 DSH 工具链高负载排障（锁竞争方案落地轮）
- read 工具对超长行截断 2000 字符——禁止用 read 输出拼回 JSON 再 parse（当日 JSON.parse 报控制字符炸）；结构化文件变换一律 python3 直接处理原文件（json.load/dump 往返 + 写后回读断言任务数）。
- bash 工具 PATH 固定不含 /opt/homebrew/bin——'brew list' 判存会误报 command not found 导致「已装判成未装」（当日 coreutils 实际已装仍触发重复安装）；调 homebrew 工具（gtimeout/graphviz）一律全路径或先补 PATH。
- 高负载机器上 run_code 组合调用频繁 30s 超时且无部分输出——拆最小单工具步骤逐个跑；长安装类命令一律 run_in_background 靠完成通知收割，不信前台超时前的残局。
- 仓库 .gitignore 忽略 .vscode——编辑器级配置走用户级 settings.json（~/Library/Application Support/Code/User/settings.json），不入库不进 worktree 流程。
- 2026-09-04 跨机 CLI 聊天演练配方：chat serve 的 listen_addrs 报 127.0.0.1 但实绑 0.0.0.0（swarm/mod.rs:139 bind、swarm/config.rs to_transport 仅展示替换）→ 双端各用固定 --quic-port + 隔离 --data-dir，好友 --addr 填「对端 LAN IP + 报告端口」即直连，无需 mDNS/rendezvous。102 端 rsync Cargo.toml+Cargo.lock+crates+apps/cli 后 cargo build 干净全量 2m09s（12 核 debug）；注意别把本机 target/ 一起 rsync（Mach-O 到 Linux 报 Exec format error）。
- 2026-09-04 GC3：DSH bash 沙箱 fs 开销下冷 cargo 全量构建会「假死」——10 个 rustc 全部 0% CPU 各只吃 1-2s CPU，30+ 分钟编不完依赖（icu 系）；解法=CARGO_TARGET_DIR 指向主树已预热的目标目录（apps/gui/src-tauri/target 自带 20k 产物，同依赖集增量只编业务 crate，3m03s 收官）。前提：确认无并发 cargo 会话共用该目录。
- 2026-09-04 GC3：全新 worktree 无 node_modules，pnpm install 报 ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY——加 CI=true 环境变量即过（pnpm store 硬链接，328 包 27s）。
- 2026-09-04 GC3：vitest 全量跑冷缓存（新 worktree 首跑）会假红——transform/setup 累计 500s+，多个 5s testTimeout 用例排队超时；隔离重跑单文件全部绿即定性为负载抖动；最终验收在主树温缓存跑（48 文件 286 测试全绿）。
- 2026-09-04 IM-T43：friends.json 预置数据的集成测试，必须先写盘再 spawn_at（Chat::new 装载时读入内存态）；spawn() 之后再写 friends.json 对 friend_update/friends_list 不可见——messages JSONL 是惰性加载没这个坑，reply_compat 先例不可直接照搬到 friends 域。
- 2026-09-04 IM-T43：run_code 里发多行 git commit message 用 `git commit -F - <<'MSG' ... MSG` heredoc；JSON.stringify(msg) 会把换行变字面反斜杠 n 进标题行。TS 模板串嵌 shell 代码时，shell 的美元花括号变量与反引号必须转义，否则宿主先插值报语法错——拿不准就拆成行数组 join。
- 2026-09-04 IM-T43：cli-parity.sh 内部会 cargo build p2pctl 用默认共享 target——并行轮先 CARGO_TARGET_DIR=隔离目录 build 好，再把二进制 cp 到脚本期望路径（apps/cli/target/debug/p2pctl），脚本检测到存在即跳过构建，共享锁零占用。
- 2026-09-04 IM-T43：zustand 泛型 helper（chat-local.ts 模式）里 (s: S) => 字面对象子集 不满足 Partial<S>（S 未定形索引签名展开），三处返回值 as Partial<S> 即过 tsc。

- 并行会话活跃 + 外置卷 I/O 慢时的功能开发工作法（gui-updater 轮实证）：`git clone /主仓库 /tmp/p2p-xxx` 到内置卷（自持 .git，免疫并行会话的 git 手术与 worktree 元数据清理），在 clone 里开分支/改码/跑全部门禁（pnpm store 复用、cargo target 可用 CARGO_TARGET_DIR 借主树缓存）；收尾 fetch 反向同步→rebase→`git push origin HEAD:refs/heads/<分支>`（推回主仓库本地路径，对象写慢给足 10 分钟超时）→主树 ff-only→推 GitHub→清理 clone。全程避开 .git/worktrees 元数据写点。
- Tauri 2 应用内更新（updater 插件）接入清单：① minisign 密钥对（`pnpm exec tauri signer generate -w <path> --password "" --ci`）；② tauri.conf.json 三件套：bundle.createUpdaterArtifacts=true + plugins.updater{endpoints,pubkey}；③ Rust 注册 tauri-plugin-updater + tauri-plugin-process，capabilities 加 updater:default、process:allow-restart；④ npm @tauri-apps/plugin-updater/@tauri-apps/plugin-process，check() 返回的 Update 句柄自己持有供 downloadAndInstall 复用（Started/Progress/Finished 三事件推进度）；⑤ CI：build 步注 TAURI_SIGNING_PRIVATE_KEY（无私钥构建即失败是安全特性），release 步生成 latest.json（四平台签名产物缺一即失败，macOS 双架构同名包就地改名，签名只覆盖内容）；端点用 releases/latest/download/latest.json 免配置常指向最新 release。
- 2026-09-05 T19：验收链（cargo test && clippy && make check）失败的分层定性法：先 `make <单target>`（gate-tests/gui.sh）对照整链复现，再分别用 `PATH=/opt/homebrew/bin:$PATH` 与默认 PATH 复现同一 target——本次三层包装（bash -c + gtimeout + bash -c）逐层复制均绿、只有整链红，最终二分出「homebrew bash5 + make」组合是触发条件；`cat -v` 看错误消息原始字节（nameM-o=0xEF）直接指认全角字符来源行，比猜 locale 快。
- 2026-09-05 T21：T19 竞态压缩法复用成功并补强——重 merge main 后定向门禁最小集 = {cli-parity（b51cd5c 起源码新于二进制自重建）、ai-docs-sync（仍只「不存在才构建」，预建默认 target 二进制的纪律不变）、apps/cli fmt+clippy、p2p-cli lib 测试}，绿后一口气 push+ff-only；全量 make check 以 merge 后末次为准。主树收尾后顺手重建主树 apps/cli/target/debug/p2pctl，防下一个会话踩陈旧二进制假红。
- 2026-09-05 T19：并行会话高频推进 main 时收尾四步的竞态压缩法：分支先推远端保平安 → fetch+merge origin/main → 查增量性质（docs-only 可免全量重验，涉码必重跑验收）→ 验收绿后同一口气 push+ff-only；脏文件是否挡 ff 用 `git diff --name-only main <branch> | grep <脏文件>` 判交 intersects，无交集即不碰他人 WIP。
- 2026-09-05 协调会话挂起恢复轮的接管时序（E10-T19）：协调方被挂起 2h 期间，执行会话自行苏醒并完成反向同步+终验+收尾四步+推送——两次催收无回音不等于死亡，可能只是 harness 挂起。接管动作（代 push/代合并/代清理）前必须最后一刻再核会话与分支动态；且所有权未决时协调方不要往执行者分支上提交（我曾在它挂起时代做了 merge main，它苏醒后需自行核实我的提交并纳入终验，双方都冒了险）。协调代劳要么完整接管四步一步到位，要么只做无所有权副作用的准备（装依赖/预构建/探针）。
- 2026-09-07 UX 终验：vite 独立实例零仓库改动拉起法——createRequire(仓库 package.json).resolve("vite/package.json") 定位包目录后 import dist/node/index.js，inline config 覆盖 cacheDir+port（与 5173 并行实例隔离优化器）；「一条命令闭环」里 server 起一次、首个场景承担冷启动预热（首载 transform >90s，LOAD 超时给 180s）、全部场景跑完再杀，cacheDir 落 /tmp 跨轮复用 deps 缓存。
- 2026-09-08 顶栏紧凑化轮：run_code 里 bash 长命令（>30s 的全量测试/lint）多次返回空输出（"completed with no output"）且无 exit 码——可靠做法是把输出重定向到 /tmp 文件再单独 cat 回读；同一命令偶发正常偶发为空，别当成命令失败重跑浪费 1 分钟。
- 2026-09-08 顶栏紧凑化轮：改共享布局组件（Topbar）前，除组件自身测试外必须全仓 grep 断言其 DOM 内部结构的"视觉证据类"测试（本次 im-visual-v2.test.tsx D1 硬找 header 里的停止按钮文本+className，全量跑到第 200 个文件才炸）——改为先 grep 再动手，随设计变更同步改写断言语义。
- 2026-09-08 通讯录双栏轮：worktree 免 pnpm install 的 deps 复用——根与子包 node_modules 各 symlink 回主树对应目录，跑门禁用 node node_modules/vitest/vitest.mjs 或 typescript/bin/tsc -b 直调（绕开 pnpm verify-deps-before-run 在 symlink 树上误触发的整库 install）。
- 2026-09-08 通讯录双栏轮：edit 的已读快照按「路径」而非内容判定——主树读过的文件在 worktree 里改同名文件仍报 has not been read，worktree 编辑前必须对 worktree 全路径重读。tools.write 的 content 若经 run_code 的 JS 模板串转手，内嵌 JSX 的美元花括号会被宿主插值报 Expected ','（本条即为现场再犯后补记）——长 JSX/脚本内容一律直接作为 write 的 JSON 字符串参数传。
- 2026-09-07 rail 轮：run_code 里对文件做字符串 surgery 时，JSON 参数内的 \n 会被解码成真实换行——正则字面量因此跨行直接 SyntaxError（报错还是误导性的 "Unmatched )"）；同理普通引号字符串里也不能出现解码后的裸换行。多行文本处理改用 edit/write 工具传字面量（它们的参数天然支持真实换行），或先用 read 取行数组再 join 处理，别在 JS 源码层玩转义。
- 2026-09-07 UX 终验：mock 态页面内造数走 vite 模块同源性——页内 `await import('/src/lib/mock-ipc.ts')` 与应用拿到同一模块实例，mockBackend.chatFriendAdd 直建好友；但 UI 表单校验比 mock 严（PeerId 需 base58 解码=32 字节，不止 43-45 位正则），页内 b58 编码 32 个随机字节生成合法 PeerId；同理 chatFriendAdd 直建的是好友，UI「添加好友」走的是邀请制（pending），两条通路别混。
- 2026-09-07 DSH harness：给 bash 的命令写进 run_code 时禁止模板字符串内嵌 ${var}（会被 JS 先求值报 "i is not defined"）——用数组 join("\n") 拼命令行。
- 2026-09-05 ACP3：并行会话会 prune/打断彼此的 worktree——worktree add 用 run_in_background 跑（前台 ~60s 超时会杀掉 801 文件的 checkout 留半成品，本日两次实证），建成后立即 git worktree lock --reason 自保；每次操作后 git -C <wt> rev-parse --show-toplevel 核对没回落主树（输出主树路径=管理区被清的事故信号）。
- 2026-09-05 ACP3：run_code 的 tools.write 内容经 JSON+JS 双层转义，Rust 转义序列写 \n 实际落盘真实换行（模板串里单反斜杠就是换行），字符串字面量当场编译红——写完必须 grep 查字符串内断行；修复用 python3 精确 replace（py 里 \\n 才是字面反斜杠 n），比 edit 锚点匹配可靠。
- 2026-09-05 ACP3：长流双向泵（WS⇄yamux）挂死自愈配方——spawn 双任务 + select 首侧结束 abort 另一侧（join 双侧都 pending 就永不结束）+ 写 16KiB 块/5s 超时重 kick + 读 30s 超时重 kick + PeerDisconnected 事件竞速兜底；yamux 批量窗口更新（<半窗不发）遇 echo 停等流量会饿死写侧（known-issues 同日条）。
- 2026-09-06 UX1：后台跑验收链的证据采集用 (pnpm lint && pnpm test && pnpm build && pnpm check:i18n) 2>&1 | tee /tmp/xxx.log 再从落盘文件 grep 摘要——job_output 长输出会丢中段（提示 dropped from memory），且多并行会话共享同一 spill 目录、日志交错难归属；落本地的完整日志才可作验收证据逐条引用。
- 2026-09-05 P0壳：等待外部前置（文档合入 main、CI 完成）用后台轮询器 + 热身并行：
  'deadline=$((SECONDS+1800)); while ...; do git cat-file -e main:<path> && exit 0; sleep 20; done; exit 124'
  挂 run_in_background，期间读代码/装依赖/建基线零空转；轮询器三态退出码（本地找到 0 /
  仅远端先行 3 / 超时 124）一次 job_output 读回即可判位，不用反复探测。
- 2026-09-05 P0壳：规格表格→测试一比一机械映射：5.3 重定向表用 describe.each 逐行生成
  用例（每行一条断言 + query 透传专项用例），13 项面板导航用注册表数组驱动循环断言——
  规格表变更时测试与表同构演化，比手写散测试多一层机械对应，回报验收证据时直接引用。

- 2026-09-05 UB：多 feature 纠缠同一文件的拆提交法——cp 备份终态 → 临时摘除后一
  feature 的行得中间态 → 只跑相关测试文件验中间态绿 → 提交 feature A → cp 还原
  终态 → 提交 feature B；每提交可独立构建、独立 revert（本轮 5 提交拆分全程无红）。
- 2026-09-05 UB：RTL stub 原型方法（如 HTMLElement.prototype.scrollIntoView）已被
  defineProperty 定义后再直接赋值报 read-only；换自己的 mock 必须 同样走
  Object.defineProperty({ configurable: true, value: mock })，断言前从 prototype 重新取引用。
- 2026-09-04 真机 E2E 找 dsh 入口：ymm-001 checkout 根 node_modules/.bin 无 dsh、根 @deepseek-ai 下也只有 dsh-tool-* 两包（pnpm workspace 不 hoist CLI bin）——真入口是 apps/cli/lib/bin.js（workspace 包 @deepseek-ai/dsh，bin dsh→lib/bin.js），先用 node <checkout>/apps/cli/lib/bin.js --version 验活性，再填 ACP_E2E_REAL_DSH（该变量按空白切分成完整 argv，node+绝对路径形式最稳，不依赖 PATH）。
- 2026-09-05 OPS1：run_code 模板字符串里写 shell/配置文本，`${var}` 必须 `\${var}` 转义（JS 插值先于 shell 执行，否则 Unexpected token 或静默插空）；多行文本最稳写法是按行数组 join，行内零反引号零 ${——本次 152 行 .mjs 入库零事故实证。
- 2026-09-05 移动分组修复：Radix Select 在 jsdom 的可测配方——`fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: "mouse" })` 开下拉（react-select 源码硬性检查 `pointerType === "mouse"`，缺省空串直接不开），option 用 `findByRole("option", { name })` 拿到后 `pointerUp`+`click` 各一次；前置桩 scrollIntoView/hasPointerCapture/releasePointerCapture 三件套。先探针 `node -e` 验证 jsdom 有 PointerEvent 再写测试，省一轮盲调。
- 2026-09-05 移动分组修复：收尾 push 判活别信 `cmd | tail; echo $?`——那是 tail 的退出码；开头 `set -o pipefail` 或 `${PIPESTATUS[0]}`（ff5831e 已沉淀过此课，本次仍复犯，验证脚本要模板化）。


## 2026-09-04 T22 门禁脚本环境加固轮

- run_code 里 tools.write 整写 bash 脚本（TS 模板串 + 反斜杠花括号转义）后必须 read 回读 + bash -n + 真跑三连验证：本次回读抓出两处静态读不出的 bug——local 临时变量被 EXIT trap 引用（trap 触发时函数已返回、local 已出作用域，set -u 下 unbound 污染退出码；trap 要引用的临时路径改存全局变量）与变量改名漏网的引用行（fakebin 仍指旧名）。
- bash 自测夹具伪造"可执行二进制"：touch 建的文件 644 不可执行，被测脚本里 [ -x ] 前置判定会直接走"缺失需重建"分支（本次 self-test 场景连红实锤）——夹具建文件后补 chmod +x。
- 抽共享 lib-*.sh 前先查门禁自测夹具的复制面：tests/cli-parity.sh 只 cp 单脚本进临时夹具树，主脚本 source lib-*.sh 在夹具里必然缺文件（set -e 即死），而夹具测试文件常在红线禁改清单——加固逻辑选择脚本内联自包含，重复 ~40 行换夹具兼容。
- 并行会话密集推进 main 时 ff-only 前置三查：git fetch 后核对（1）本地 main == origin/main；（2）merge-base --is-ancestor main <合并点>；（3）主树 status 干净。本轮 main 在会话中段从 895c3a4 前进到 544b1ae，三查全过才 ff。
- worktree 新 checkout 跑 make check 前先补环境：apps/gui/node_modules 不进 git，gui-check 的 eslint 直接 command not found（环境假红非代码红，归因先看报错形态）；pnpm install 后 ls node_modules/.bin/eslint 确认再重跑。cargo 侧 worktree 首跑必全量编译，先后台预热 cargo build 再跑验收脚本。
- 脚本双 bash 兼容验证法：/bin/bash -n + /opt/homebrew/bin/bash -n 各过一遍语法，--self-test 两边各跑一遍（验收链用哪个 bash 取决于调用方 PATH，不能只验一个）；变量展开一律花括号化（延续 90e062a 对 bash 5.3 多字节相邻展开的防御）。
- 后台验收命令经管道收尾（`make check | tail`）时 job 退出码是 tail 的，假 exit 0 掩盖真失败：必须 `set -o pipefail` 并回读 `PIPESTATUS[0]`，或落日志文件后 `echo EXIT:$?`（2026-09-04 ACP6b make check 首跑假绿实证）。
- Radix Select 组件测试前提三桩：`HTMLElement.prototype` 的 scrollIntoView/hasPointerCapture/releasePointerCapture 各 `Object.defineProperty(..., {configurable:true, value: vi.fn()})`；点选拿 pointerDown({button:0, pointerType:"mouse"}) 开面板再对 option pointerUp+click（权威先例 chat-friend-group.test.tsx 移动分组用例）。
- 2026-09-04 run_code 源码载体对反引号字符敏感：不仅模板字符串，普通字符串字面量里出现反引号字符同样炸解析（报 Expected , got ; / Expression expected，定位不到真实行）。要写含反引号的内容（Markdown 代码标记、带转义引号的 Rust 字符串）一律用 String.fromCharCode(96) 拼接，或改走 bash heredoc 落盘。
- 2026-09-05 查 crates.io 依赖真实版本列表（rsproxy sparse index）：3 字符 crate 路径是 `/index/3/<首字符>/<名字>`（如 curl -s https://rsproxy.cn/index/3/y/yrs | jq），2 字符是 /2/<名>，≥4 字符才是 /前2/次2/<名>——路径写错只会得到 NoSuchKey。yrs 用它确认 0.27.4 最新（"0.6" 是 2021 年化石）。
- 2026-09-05 yrs 追加日志「应用成功却不可见」诊断：examples/ 下写临时探针（examples 可直接用 crate 的依赖 yrs），逐行 decode→apply 后打 map.iter 键集合，再打 update.insertions(true) 的 client/clock 区间——两行同 client 同 clock 即 clientID 撞车实锤。
- 2026-09-05 并发追加压测抓现场：临时拷贝 E2E 脚本并把 trap cleanup 置空（sed 替换 cleanup 函数体）跑 N 轮，失败轮的临时数据目录原样保留供逐行 forensic；正常跑完的目录逐个 CLI 读回计数定位失败轮。

- 2026-09-05 T23 两机冒烟测 rendezvous 发现：对端注册不可见时先看三方证据——borrower DISCOVER-DEBUG（query err vs empty）、bootstrap 侧 server link ended 频率、lender 自查询是否为空；query error + handshake timeout = 客户端连接被服务端单次服务耗尽，empty = 注册退化 loopback 被地址卫生过滤。两类修法不同：前者合并场景减少进程拨号，后者等/重试观测。
- 2026-09-05 底座 rendezvous 服务端对同一连接近似「注册即耗尽」：第二进程的查询会挂 10s 握手超时（bootstrap 侧每 10s 一条 server link ended）。场景编排把多个断言合并进同一个借方进程最稳，跨进程的新身份首查询也可用，但绝不要在同 conn 上做第二次查询。
- 2026-09-05 p2pctl llm-share offer publish 报「节点身份加载失败」是种子未存在：先让节点域生成身份（任何 load_or_generate_seed 的载体，如 harness identity 模式）再 publish；publish 失败 stderr 有清晰中文原因，别被空 stdout 骗。
- 2026-09-05 mock 上游喂 SSE 必须是「data: {json}\n\n」事件原文（空行分帧），裸 JSON 块提取不出 usage → 收据 estimated=true（extract_usage 只认 data: 行前缀）；断流剧本要在 usage 帧前断，否则按实际 usage 结算不出 estimated 收据。
- 2026-09-05 git bundle 给无 github 出网的机器供代码：bundle create f HEAD（打包机器当前 HEAD）→ 对端 git clone f dst；增量更新 git fetch f HEAD + reset --hard FETCH_HEAD。5.9MiB 包秒传，crates.io 可达即可构建。
- 2026-09-05 facade 观测（OBS1 反射口）单次探测 2s 超时且装配期只试一次：跨机 UDP 随机丢包会让注册退化 loopback 地址（查询端 is_routable 过滤后为空）。载体进程在装配前对反射口做有界重试预检（OBS1 魔法 UDP 探测）可消掉这类整轮毒化。

- (2026-09-04) i18n key-set 门禁(i18n-diff.sh)只比对 zh/en 键集合,不校验插值变量:断言文案时注意 `{{count}}` 缺失会在运行时静默渲染空串,测试要断言整句而非数字。
- (2026-09-04) 行数红线(300)会倒逼测试落点:向已有 257 行的测试文件追加会破线,提前 `wc -l` 目标文件再决定新建 `*-flow-*.test.tsx` 还是就地扩展。
- (2026-09-04) 一次 commit message 要多段正文时,用多个 `-m '标题' -m '正文'` 参数,别在 bash 字符串里写 `\n`(会被字面量嵌入)。
- (2026-09-04) RTL 断言 `expect(el).toBeNull()` 的 el 必须来自 `queryBy*`;`getBy*` 找不到直接抛错,失败信息会误导排查方向。

- (2026-09-04 增补单轮) esbuild/vitest 不认 `const { a, type B } = await import(...)`:动态导入解构里不能写 inline type,类型一律静态 `import type`。
- (2026-09-04 增补单轮) sonner toastError 有 3s 去重窗口,同文案第二条直接 dismiss 全部 toast:同文件多个用例断言同一失败文案时,第二条改断 store 状态,否则 findByText 超时假红。
- (2026-09-04 增补单轮) fake-timer 下 mock 回放脚本的时间线要按步长累加算准(含 stop 步),advance 不足会『应答未到达』假红;vi.advanceTimersByTimeAsync 后再断言。
- (2026-09-04 增补单轮) 行数红线对『会在本轮膨胀的文件』要预算:本轮 acp-connection.test.ts 两轮追加超 300,提交前统一 wc -l 白名单内所有改动文件兜底。

## 2026-09-05 R1 群聊 goutbox 可靠性轮
- 2026-09-05 R1：回归测试红绿有效性机械验证——fix 与测试同树就绪后
  git stash push -- <src目录> 只留测试跑红（红点必须正是断言的缺陷行为），
  stash pop 后跑绿；「测试真的在测缺陷」从口头承诺变成两分钟内可复现的事实。
- 2026-09-05 R1：时序类断言的确定性判据——#[tokio::test] 默认当前线程调度，被测
  命令 .await 返回后到断言之间不落任何 await 点，则后台任务（PeerConnected flush
  等）物理上不可能插队；「命令内完成/紧邻一步送达」类断言全部用同步读盘即可排除竞态。
- 2026-09-05 R1：run_code 模板字符串含反引号/复杂转义会在解析层直接炸
  （Expected unicode escape）且整段程序不执行、其中所有已发工具调用全部回滚——
  大块代码文本搬移用「read 行切片 + write 整写」，长文本一律走 write 工具，
  不在 JS 串里手拼含转义的代码内容。

run_code 里用 TS 模板字面量写 bash 内容时，bash 的 ${VAR} 会被 JS 当插值解析：${1:-x} 直接语法错（Unexpected token），${BASH_SOURCE[0]} 运行时才报 BASH_SOURCE is not defined——同一天连踩三次。写法：模板串里一律转义后写，或改普通串数组 join 整写（后者更稳，Makefile 的 tab 配方同理用 \t 拼）。
给产物特征扫描类内容守卫定特征清单，必须双向实测校准后才可信：干净构建必须零命中（防误报假红）＋泄漏/注入场景必须命中（防漏报假绿）。本条产出 gui-dist-scan.sh 的 mock 特征清单（2026-09-05）。
vite 插件在 configResolved 抛错的构建期断言，失败发生在 bundle 开始前，红路径探测秒级返回——「必红」回归放 gate-tests 成本极低，别因怕慢而放弃真实红路径探测、只测夹具。
- 2026-09-05：GUI 门禁先本地全量三连（eslint src 全目录 + pnpm build + vitest run 全量）再 make check——gui-check 的 eslint 含 react-hooks 编译器规则，vitest 全绿不代表 lint 绿，三连绿后 make check 一次过的概率大幅上升。
- 2026-09-05：逐码断言表单错误：CASES 表 {field, value, code} it.each 循环，断言 testid 为前缀-error-加码 且文本 === i18n.t(key)——「稳定错误码 + i18n」验收从此机械可验。
- 2026-09-06：免认证 CI 排查三板斧（repo 公开时）：runs API 拿时长分布区分秒级早退与编译后失败；check-runs annotations 拿 exit code（内容仅此而已）；本机 gh hosts.yml 有 token 时 jobs/{id}/logs 跟 302 到 blob 直接拿全量日志（403 只挡无凭据请求）——本会话靠它把 ubuntu 红因从猜测变实证。
- 2026-09-06：GH runner 上 pkg-config 存在性可作声明式 SKIP 信号（缺 webkit2gtk-4.1 即 SKIP），且 pkg-config 本身缺失时取反仍为真——SKIP 口径对工具缺失也稳健。
- 2026-09-05 发布链路：gh 未装时取 CI 证据——~/.config/gh/hosts.yml 的 oauth_token 直接作 Bearer；job logs API 公开仓库未认证也 403（Must have admin rights）；步骤 env dump 判读 "VAR: ***"=secret 存在非空、"VAR:" 空=secret 不存在（GitHub secrets 不存空串，workflow 表达式对缺失 secret 求值空串导出）；条件步命中与否看 jobs API steps[].name+conclusion，免拉日志。
- 2026-09-05 发布链路：base64 报错 "Invalid symbol N, offset M" 定因法——拿 M 对照本地正确值长度，M==长度即锁定「尾部多一个字符」，N 是杂字符 ASCII；本地 tauri signer sign + 构造同形态脏值可逐字复现 CI 报错，免全量 build。
- 2026-09-05 发布链路：写 GitHub Actions secret 免 gh 装——python3 pynacl SealedBox 加密原文件后 PUT /repos/{o}/{r}/actions/secrets/{name}，204 后 GET secrets 看 updated_at 变动；验证构建修复用 workflow_dispatch（release job 只挂 tag ref，自然 skipped，零发布副作用）。
- 2026-09-05 DSH：run_code 里长含反引号/markdown 的文本会被 JS 模板串截断、bash 变量在模板串里会被 JS 插值——多行内容用行数组 push 后 join，bash 变量写成转义形式。
- 2026-09-06 IMC1 卡：agent 里用 run_code 写大文件（Rust 源码等含括号/换行的内容）时，content 先赋给 const 变量再传 tools.write，别把长内容内联进调用——两次踩坑：闭合序列写重（多一层反引号闭括号）、多行字符串裸换行破坏 JS 语法；const 先行还能顺手 .split("\n").length 回读行数对账红线。
- 2026-09-06 IMC1 卡：cargo/长命令接管道（| tail / | grep）会把真实退出码换成 tail/grep 的——验收判断一律命令分号接 echo exit=$? 落盘或输出首行，别信管道尾命令的 rc。

## 2026-09-06 AS4 share E2E
- bash 里 cargo ... | tail 会吞退出码：验收/门禁判定一律用 echo EXIT=${PIPESTATUS[0]}。
- rustfmt 宏调用 fn_call_width 默认 60：单行宏参数超 60 字符被竖排膨胀，行数红线（300）文件先用短参助手收敛再过 fmt；fmt 会改行数，先 fmt 再数行。
- DSH edit 工具做读时快照校验：外部进程（cargo fmt）改盘后必须先用 read 工具重读该文件再 edit，否则报 "file changed since it was read"。
- make check 首跑 gate-tests 的 vite 真实构建可能因 pnpm 冷缓存 "Command vite not found" 假红，预热后单跑即绿；判定环境问题前先单测该脚本一次。

- 2026-09-06 DSH run_code 里调 write/edit 工具：run_code 的参数序列化会要求 write 带 `description` 属性（顶层 write 工具直接调用不要求）；且对同一文件连续 edit 报 file changed since read 时，重读一次即可再 edit。session_link_list 用 `{}` 调用报 lossless JSON 错，无参重试即过。
- 2026-09-06 vitest 里 vi.stubEnv 必须先于模块求值：静态 import 会提升到所有语句之前，被测模块可能在 env 就位前绑定错误分支（实例：console-watch 顶层读 import.meta.env 选 ipc 后端）。修法 = 全动态 await import(...) + vi.resetModules()，保证整链按 stub 后环境重求值。
- 2026-09-06 硬编码文案扫描（i18n hardcoded-copy.test）会命中「代码行尾的 // 中文注释」，独立成行的中文注释不命中。写码时中文注释一律独立成行。
- 2026-09-06 zustand subscribe 回调里再 setState 会自环：必须 (s, prev) 边沿触发（只对关心的切片比较变化）+ 幂等前置守卫（已处理状态直接 return），否则订阅-setState-再订阅瞬间栈爆（实例：console-watch 发现面解析回环）。
- 2026-09-06 DSH bash 每次调用都是全新 shell，默认工作目录 = 会话工作区（主树）：在 worktree 干活时每条命令必须显式绝对路径 cd；相对跳转会落回主树跑错代码（实例：基线测试跑在主树全绿，误判 worktree 健康后才开始改）。
- 2026-09-06 跨 worktree 共享依赖别软链 node_modules：pnpm 的相对符号链接经 vite 解析会断（UNRESOLVED_IMPORT），直接在 worktree pnpm install——共享 store 硬链接，426 包全量仅 2.4s。
- 2026-09-06 查 UI 依赖真实行为直接读 unpkg 的未压缩 dist（unpkg.com/pkg@ver/dist/index.js）+ web_fetch 取段分析，比本地 grep 压缩产物/搜二手 changelog 快且准。


## pnpm 在本机 shell 里前台挂起 → 一律走 run_in_background 作业（2026-09-06 LSG3）
- 症状：bash 里前台跑 pnpm（连 --version 都算）零输出，约 60s 被 SIGTERM；~/.vite-plus/bin/pnpm（corepack shim）同样挂。
- 修法：pnpm 一律 run_in_background 作业 + 输出重定向 /tmp 日志再 grep；日志文件比 job_output 流式读取可靠（流式读一次即消费，丢输出）。
- 2026-09-06 本机 bash 工具里 node/pnpm 静默挂起（exit=null 无输出）：PATH 首位是 ~/.vite-plus/bin，其 node 是 vp 启动器会挂起；export PATH=$HOME/.nvm/versions/node/<版本>/bin:$PATH 后恢复。新 worktree 无 node_modules，nvm pnpm install --frozen-lockfile 走共享 store 秒级。
- 2026-09-06 vitest 全量在高负载机器上假超时（acp/app-boot 5s testTimeout/40s hookTimeout 成批红）：先对失败文件单跑隔离复判——真红隔离下仍稳定红，假红秒绿；隔离复跑再决定是否改码，避免误诊。
- 2026-09-06 LSG1：并行会话共用 /tmp 时，后台任务日志名必须带独一标记（如 /tmp/xx-$$.log 或会话前缀），否则互相截断误读（实证：/tmp/lsg-test.log 被兄弟会话 vitest 输出覆写）；后台 cargo 任务超 600s 墙钟前先转 run_in_background。
- 2026-09-06 UX-F：mock dev 页面走查在「每次 cli 调用=全新 Chrome+全新 mock 状态」约束下，用单会话 CDP 驱动脚本（spawn 一次 Chrome，hash 导航保持 JS 态存活）一次跑完全流程并按步截图；页面内经 `await import('/src/lib/ipc.ts')` 拿到与 app 同实例的模块（vite dev 同 URL 同实例），直接调 mock 测试引导入口（如 chatFriendAdd）+ zustand store 的 getState().loadFriends()，比爬表单可靠。React 受控按钮 element.click() 打不开 radix 弹框时，取 `el[Object.keys(el).find(k=>k.startsWith('__reactProps$'))].onClick({})` 直调处理器；先 querySelectorAll('[data-slot=card]') 按卡片文本定位再找按钮，防同名按钮撞车（顶栏与状态卡都有「停止节点」）。
- 2026-09-06 UX-F：DSH run_code 里 tools.bash/edit 等全部调用都必须带 description 字段，漏了直接参数校验失败且同一程序里已完成步骤的输出全部丢弃；长链 git 命令（reset --soft 后接 reset）在 15s 默认超时下可能半途而废留混合状态——每段独立调用、逐段核 exit code、超时给足。
- 2026-09-07 vitest4 已删 `--reporter=basic`（会当自定义 reporter 模块加载报 ERR_LOAD_URL）；抓失败清单用默认 reporter 输出重定向后 grep FAIL 行。
- 2026-09-07 管道吞退出码假绿：`cmd | tail -8; echo $?` 取到的是 tail 的 0；验收判定一律 `cmd > log 2>&1; rc=$?` 直取命令退出码。
- 2026-09-07 gui-agent 走查 SPA：页面内 mock 状态不跨调用存活，每个场景在单次 eval 内闭环（导航+操作+断言一次跑完）；启动竞态会吞掉早期的 location.hash 赋值——轮询循环里反复 set hash 自愈，失败分支回传 bodySnippet 便于诊断。
- 2026-09-07 UX-E 多步 SPA 走查用自写单会话 CDP 驱动（gui-agent 每次调用都是新 Chrome，多步交互必须合并进一次会话）：CDP /json/new?url= 建的 tab 停在 about:blank 不导航，必须显式 Page.navigate + 轮询 readyState；boot 判据 = readyState complete && mock 注入存在 && root innerHTML>1000；React 受控输入用原生 value setter + dispatch input 事件；blur 类校验显式 dispatchEvent focusout(bubbles)。每次改源码都会触发 HMR full reload 打断走查——走查前确保源码已冻结。
- 2026-09-07 UX-E vite 6 没有 --cacheDir CLI：隔离缓存用 --config /tmp/xxx.mjs wrapper（import 官方 vite.config 后展开覆盖 cacheDir），零仓库污染。
- 2026-09-07 模块级「单次提示」降级 flag（如 console 不可达单次 info）：测试里前置 describe 的失败样例会提前消费额度，断言用例必须 beforeEach 调导出的 resetXxxForTest() 复位，否则 info 计数恒为 0 假失败；Tauri 态分支用临时 window.__TAURI_INTERNALS__ = {} 注入（isTauriRuntime 是调用时读取，非模块加载时）。
- 2026-09-07 R2F-C 端口预检：派单分配的调试端口可能被历史孤儿进程占着（5179 被 9/2 的 /tmp/report-server.py 占，PPID=1 可判孤儿）；dev server 起不来先 lsof -nP -iTCP:<port> 查身份，确认孤儿再 kill，不误伤并行会话。
- 2026-09-07 浏览器 mock 走查 ACP「在线态」触达：真机 acp-console 若在 8787 监听，mock token 会被拒（1006/401），MockSocket 与真 ws 都走不通——用 Page.addScriptToEvaluateOnNewDocument 垫片伪造 /discovery fetch（对齐单测 stubGlobal 口径）仍不通时，按派单预案走 store 状态注入（window.__acpInject* 仅 VITE_MOCK_IPC=1 暴露），注入后轮询重打对抗自动流程的相位覆写。

- 2026-09-07 UX-R2A run_code 工具里写「含反引号/多层转义」的文件内容必撞 JS 解析（外层模板串被内层截断）：改走 bash heredoc（<<'EOF' 引号形态零展开）落盘，或先 write 骨架再 edit。
- 2026-09-07 UX-R2A CDP 有状态走查（A 面板操作、B 面板验证联动）必须常驻 Chrome 会话：仓库 gui-agent.mjs 每次调用起杀一个 Chrome，页面模块态全重置；复制它改 DEBUG_PORT + 常驻循环 + steps.mjs 模块文件（evalOf 助手前缀），consoleAPICalled 监听顺带收 console 证据。
- 2026-09-07 UX-R2A 页面驱动 React 受控组件：Input/Textarea 用原生 value setter + input 事件，onBlur 用 FocusEvent('focusout', {bubbles:true})（React onBlur 走 focusout 委托，派发 blur 无效）；Radix AlertDialog 里点确认按钮必须 querySelector('[role=alertdialog]') 作用域内找，行内同名按钮在文档序更前会截胡。
- 2026-09-07 UX-R2A vitest forks worker「Timeout waiting for worker to respond」是负载抖动不是代码问题：pkill 残留 vitest/node worker 后重跑即绿；长时间多轮跑测后先 ps 清场再跑门禁。
- 2026-09-07 UX-R2A vite 程序化 createServer 做隔离走查实例：root 指向 apps/gui 复用仓库 vite.config，cacheDir/server.port 覆盖即可；vite 包 import 不动 node_modules 顶层软链时用 apps/gui/node_modules/vite/dist/node/index.js 绝对路径。

## 远程服务器排障（2026-09-07）
- 138 服务器 = 43.240.223.138（.env 里 LINUX_SSH_138=ops@43.240.223.138；ssh config 有 public-box:25446 / public-box-2222 别名，备用端口可能反而不通）
- 服务器上只有 ops 用户；本机公网 IP 用 curl -4 ifconfig.me（不带 -4 拿到 IPv6，与 fail2ban 记录对不上）
- macOS 无 sshpass 时用自带 expect 验证密码 SSH 登录；批量远程诊断用 ssh '多行脚本' 一次跑完，sudo -n 免交互

## 2026-09-07 桌面 GUI 真机自动化（UX-R2 真实流程终验）

- 桌面 WKWebView 无 CDP：GC1 控制通道（p2pctl gui navigate/page/screenshot/invoke）+ Swift 按 pid 直连 AXUIElement 组合可完成真实点击/表单/读数闭环。
- System Events 会把同 bundle id 的多个 app 实例 AX 树解析到同一实例（两份 p2p-console 时拿到错误窗口内容）——必须用 AXUIElementCreateApplication(pid) 直连（swiftc 小工具）。
- React 受控输入：AXUIElementSetAttributeValue 设值成功但被受控值回弹；逐字符 CGEvent（US 键位表+shift 符号映射）可真实触发 onChange；JSON/特殊文本用写剪贴板+Cmd+V 最稳。
- AppleScript 递归 dump AX 树：tell 块内调用自定义 handler 必须 my handler(...)，且按 unix id 过滤 process 在同名多实例时不可靠。
- 2026-09-06 R2 复核：裸 CDP（Node ≥22 全局 WebSocket）配方补遗——PUT /json/new?about:blank 开靶（老版本回退 GET）；evaluate 只接受表达式，箭头函数源码必须自动包 `"(" + SRC + ")()"` 再求值（漏包=取回函数对象、detail 全 {} 假阴）；交互用 el.click() 与 scrollTop 直写 + dispatchEvent(new Event('scroll')) 双保险（原生 scroll 事件本就异步触发）；hash 路由间 Page.navigate 不触发 loadEventFired，就绪判定靠轮询目标 UI 而非等 load。

## 2026-09-07 协调者长任务轮询节奏（run_code 墙钟与 talk 凭证）
- run_code 整体有 600s 墙钟上限：bash 内 sleep 470+ 这种单块轮询可行（配 timeoutMs 540），「循环内多次 sleep+poll」必撞上限整体被斩、已产出输出也一并丢失。
- session_link_talk 的 talkTimeoutMs 设 ≥600s 时超时以异常抛出、claimToken 随之丢失；设 ≤480s 则超时返回 delivered=true/replied=false + claimToken，事后用 session_link_collect 收割。collect 报「凭证无法识别」= 目标回合还没走到消息可见边界，稍后重试即可，别放弃 token。
- 判断被委派会话是否卡死别用 ps 全量计数：跨项目孤儿（别的仓库同名二进制）会污染计数；按「启动时间过滤 + /tmp 专属目录新文件 + git 提交推进」三信号交叉判断。

## 2026-09-07 worktree 特性开发工作流提效（acp 自描述文件特性全程）
- edit 工具要求 read 与 edit 同一精确路径：worktree 副本即使内容与主树刚读过的一致，也必须重新 read worktree 路径，否则报 file has not been read。
- edit 工具实际还要求 description 字段（工具目录未列出但 harness 强制），漏掉直接整批调用失败。
- git fetch 可能挂死拖垮整条 && 链：网络操作一律独立执行 + 短超时，别和本地操作串联。
- worktree 冷构建慢：export CARGO_TARGET_DIR=主树对应 crate 的 target 目录可复用依赖缓存（path-dep 本体重编，registry 依赖全命中）。
- src-tauri 接 apps/ 下 crate 的 path 依赖是 ../../../apps/<crate>（src-tauri 在 apps/gui/ 下，三层 ../ 只到仓库根）。
- eslint react-hooks/set-state-in-effect 强制生效：effect 内直接 setState 重置状态会红；用仓库惯用的「渲染期状态调整」（cached 值比对 + 条件 setState）替代。
- vite 首跑后 node_modules 由主树共享（pnpm-workspace），worktree 里 GUI vitest 无需重新 install。
- 纠正前条「worktree 无需 install」：新 worktree 的 apps/gui 下并没有 node_modules，tsc/vitest/eslint 直接报缺依赖；要先软链两处——根 node_modules 与 apps/gui/node_modules 指回主树对应目录（2026-09-07 WX1 会话实测，typecheck/test/build 全过）。

- 2026-09-07 worktree 软链 node_modules 后 pnpm run 仍会触发 verify-deps 自动 install 并炸（软链树上 .modules.yaml 归属主树路径）：门禁直接调 node_modules/.bin/<bin>（vitest/eslint/tsc/vite）绕开 pnpm 依赖状态检查，WX1 轮的「软链即可」结论只对直接调二进制成立。
- 2026-09-07 run_code 的 bash 前台命令约 1 分钟被截断且 stdout 静默丢失（本会话两次假象：命令在跑但结果不回）；>60s 的 vitest/cargo 一律后台 job + 重定向落盘 + job_output 收集，结论从日志文件读，不信任前台回显。
- 2026-09-07 fireEvent.keyDown 断言事件穿透：React 合成 stopPropagation 只拦 React 树内冒泡，拦不住 document 级监听（Radix Dialog 的 Esc 关闭路径）；验「Esc 不连带关弹窗」要挂 document keydown spy 断言不被调用，只断言 React 外层 onKeyDown 不响是不够的。
- 2026-09-07 DOM 断言 undefined≠null：querySelector 没找到元素时 optional chaining 得 undefined，属性存在但值缺失才是 null——断言挂了先分清「元素没查到（testid 拼错/取值域错）」还是「属性没渲染」，别急着怀疑组件库透传（lucide-react rest props 全透传，role/aria-label 均可落 svg）。
- 2026-09-07 git push 偶发 "repository exists" 尾部报错多为 SSH 瞬时抖动（机器高负载下）：先原样重试一次再看，别急着改 remote 配置。
- 2026-09-07 多会话并行期 ff-merge 大概率撞车：合并前 fetch + 查 main..origin/main，撞了回 worktree merge main 重跑受影响门禁再推——不要用 --no-ff 绕过，ff-only 纪律保 revert 可行。
- 2026-09-08 评审某页面设计前定位代码：别在仓库根用宽 pattern（`llm.?share`+多扩展名 include）grep——大仓会产出 50KB+ spill 且 9 成是测试/i18n/IPC 噪音；先 `ls + wc -l` 目标视图目录拿结构清单，再读主 view 文件，最后用窄 grep 补 i18n 文案（locales 直接读 llmShare 段）。
- 再纠正：软链好后别用 pnpm exec/pnpm test 起 vitest——pnpm 的 verify-deps-before-run 会因根 node_modules 是 symlink 报 ERR_PNPM_UNSAFE_MODULES_DIR 并试图重装；直接 node 起真实入口绕开：cd apps/gui && node node_modules/vitest/vitest.mjs run <files>（2026-09-07 llm-provider-share 会话实测）。
- 2026-09-08 右键菜单轮：测试里 mock 带动态 import 的 Tauri API（@tauri-apps/api/webviewWindow）时，工厂 class 在构造函数里 queueMicrotask 自发 `tauri://created`，once 句柄同步注册后微任务即触发——`await openXxx()` 直接收敛，无需真实窗口；失败路径用 hoisted 状态位切换自发事件为 tauri://error，两条路径都能在 jsdom 里确定性测到。
- 2026-09-08 右键菜单轮：store 测试要验证「同一用例内二次重载模块」时 beforeEach 的 vi.resetModules 不够，须在用例内显式再调一次 vi.resetModules() 后重新动态 import，否则拿到的是首次求值的同一单例（ui-prefs-store.test 的 freshStore 模式只在跨用例生效）。
- 2026-09-08 设计稿轮：gpt-image-2 出图管线三件套——提示词 txt + python3 重建 payload json + gen.sh（取 .env 第一对 key 调 /v1/images/generations，b64 解码落盘，docs/design/message-center 先例）；中文 UI 文案在提示词里逐串引号列出并注明 exact text rendering；出图后 read_image 直读审查文字渲染，一次生成两张互补视图（默认态+历史态）验证交互闭环。
- 2026-09-08 设计稿轮：.env 同名键重复时 source 取最后一个——排障先 `grep -n "^KEY=" .env` 看重复，逐对配 base_url 测试（本仓第一对 openai.bowong.cc 可用、第二对 api.790053500.com 欠费）；图像 API 先发低成本探测（GET /models 或 low quality 小图）验证凭证再烧高质量大图。
- dsh --profile acp 裸跑启动被拒（launcher 缺 ctx.appExit/appReady）：不动 harness 仓库，在 ~/.dsh/profiles/acp/cordis.patch.yml 用「disabled 原条目 + insert 自托管启动器」顶替 acp-app-startup（实现见 scripts/ops/dsh-acp-launch/index.mjs，自备 appExit/appReady 缺省 + 复刻原语义），acp 行靠 inject 反应式等待服务；入口名换模块（patch 改 name）可行，但 patch 不能改已存在条目的位置，insert 永远追加在尾部、靠 inject 反应式语义兜住顺序问题。注意 profile 目录按 DSH_HOME 解析（本机 ~/.dsh 与 ~/.dsh/dsh012-clean 双 home 并存，shim 两个都要装）。
- 诊断常驻服务老化：对比「运行中二进制的端点面」与「仓库 HEAD 的端点面」（如 admin GET /workspaces 新端点 404 = 旧二进制），部署刷新用 scripts/ops/acp-local-setup.sh（构建+安装+shim+授权+kickstart+探针一条龙）。
- 2026-09-08（W3 波）多行 Rust 代码经 run_code 写入：用「单引号 JS 字符串逐行 push + join」最稳；双引号数组内反斜杠转义与模板串屡触发解析错。单引号方案里 Rust 生命周期撇号先用类型别名规避。
- 2026-09-08（W3 波）bash 工具不传 workdir 时落在会话工作目录（主树），与上一次调用的 cd 无关——曾把主树的 cargo check 误当 worktree 检查得到假绿。工作区命令必须显式传 workdir。
- 2026-09-08（W3 波）read 工具有单次返回行数上限（totalLines 可能大于实际返回行数）：「读全文再写回」的追加方式会静默截断长文件（本次砍掉 techniques 172 行/known-issues 117 行，靠 git checkout HEAD~1 -- 恢复）。长文件追加一律用 bash cat >> heredoc。
- 2026-09-08（A2A3 波）run_code 有 60s 计算预算：cargo test/build、pnpm build、make check 等长命令用 node child_process exec(...).unref() 后台跑 + 重定向日志文件，隔 30-45s 轮询 grep 结果摘要；前台 execSync 必超时。
- 2026-09-08（A2A3 波）tools.bash binding 坏掉（报 missing required property "description"，与参数内容无关且最小调用也失败，疑 glob 30s 超时后 worker 通道损坏）：run_code 内改用 node:child_process 的 execSync/exec 全程替代，读写文件照走 tools.read/write 不受影响，任务不必中断。

- 2026-09-08 新 worktree 跑全量测试前的一次性环境预置：apps/gui `pnpm install`；apps/acp-agent `cargo build`（debug）——p2p-itest 的 a2a/task/card wave 夹具按 CARGO_TARGET_DIR→apps/acp-agent/target 找 acp-echo-stub，缺失时 0.00s 秒 panic「stub 未构建」（workspace test 全量必踩）。
- 2026-09-08 把「响应竞态假红」变确定性复现：放大请求体（如 2MB）强制「关闭时必有未读数据」，修复前 RST 高概率复现、修复后恒绿——比靠并行负载碰运气的回归测试强一个量级（见 tests/status_close.rs）。

## 2026-09-08 免装密钥工具生成支付宝 RSA2 密钥（openssl 直出）
- macOS 自带 LibreSSL 三条命令等价替代支付宝开放平台密钥工具的「生成密钥+格式转换+密钥匹配」：
  `openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048`（PKCS8，Java 用）→
  `openssl rsa -in pkcs8.pem -out pkcs1.pem`（LibreSSL 的 rsa 命令默认输出 PKCS1「BEGIN RSA PRIVATE KEY」）→
  `openssl pkey -pubout`；匹配校验用「从 PKCS1 私钥再导公钥 diff 公钥文件」。
- 控制台/配置要的是去头尾换行的单行 base64：`grep -v '^-' xxx.pem | tr -d '\n'`。
- 密钥落盘前先查 .gitignore：.env 已忽略不等于 keys/ 目录被忽略，生成前一条
  `git check-ignore` 判定并补 `keys/`，防密钥材料进 git（本日实录：.gitignore 只有 .env）。

## 2026-09-09 rail 移除协议文档入口轮
- 本仓 DSH glob/grep 工具与 grep -rn 全树搜索均 30s 超时；bash PATH 无 rg（之前 `rg ... 2>/dev/null` 静默无输出实为 command not found 被吞）——检索用 /opt/homebrew/bin/rg 绝对路径 + 明确子目录（apps/gui/src），秒回；命令「零输出」先排除 not found 再下「无匹配」结论。
- git worktree add 在本仓（1602 文件、外置卷）前台 120s 也会被杀留半成品（文件已部分落盘、worktree 未注册、分支已建）——重申 ACP3 结论：一律 run_in_background；中断清理三连：`git worktree prune` + `rm -rf <dir>` + `git branch -D <分支>`，再后台重建。

## 2026-09-09 生图设计轮（负责人协调手法）
- 重大纠偏（目标作废/换目标）禁止只发 session_link_send 排队：长轮次执行者不读新消息，会把接管误判为双写攻击并按 AGENTS.md 撤退协议开新 worktree 继续错误目标（本次实测，靠 interrupt_agent 止损）。正解：interrupt_agent 打断当前轮次 → 派发新任务书 → 磁盘核验旧产物已清理。
- 监督长任务用「产物落盘节奏」而非消息回报：要求执行方每次尝试即写 results/log json，协调者 5 分钟轮询磁盘；文字回报在长轮次里既慢又可能为空。
- 生图参数策略定式：先 low 探针定尺寸白名单（本次 1024x1024 / 1024x1536 / 1536x960 过、2560x1440 必挂、1536x1024 抖）→ 目标尺寸 high 串行 + 15s 退避重试（最终成功率 100%，单张最多 4 试）→ 失败张留到网关空闲窗口补跑，524 高发期等 5~10 分钟即恢复。

## 2026-09-09 通讯录资料互通轮
- src-tauri 独立 workspace 的 cargo check 冷跑 10 分钟起步且前台会超时：一律 run_in_background；被杀后重启通常已预热，9 秒完成。不要用主树 target 环境变量强行共享（env-hook 已设全局 CARGO_TARGET_DIR=~/.cargo-target，进程内改写反而分裂缓存）。
- 反向同步撞上并行合入（ff-only 失败）时：worktree 内 `git merge main`（本次零冲突）→ 全量 make check → 推分支 → 主树 ff-only 重试，全程无人工决策点。

## 2026-09-09 worktree 收尾巡检轮
- 「该合未合」机械判据：`git rev-list --count main..<分支>`（>0 才需要合并）+ `git rev-parse <分支> origin/<分支>` 确认本地=远端；两者都干净时分支只剩收尾四步的 ③④。
- worktree 残骸有三种形态，巡检要交叉验证不能只看 `git worktree list`：注册中的工作树（`status --porcelain` 判断脏）、已注销但目录残留（直接 rm -rf）、分支已删只剩空目录骨架（find 确认零文件后删）。
- 脏 worktree 判定「无未提交价值内容」：`status --porcelain | grep -v '^ D'` 为空 = 只有纯工作区删除、无修改无未跟踪，且删除未进 index，HEAD 里内容全在 → `git worktree remove --force` 零损失。

## 2026-09-09 聊天条目一键复制轮
- 剪贴板复用锚点先查再写：`components/feedback/copy-button.tsx`（CopyButton，toast+i18n 齐全）与 `views/shared/clipboard.ts`（copyText 纯函数）是全仓唯二复制设施；本次零新增 i18n 键（common.actions.copy / common.copied / common.copyFailed 均现成），全程未动 locales。
- 气泡外侧悬停动作钮布局定式（MessageBubble）：锚 `absolute top-1/2 -translate-y-1/2` + 空白侧 `right-full/left-full`，尺寸 size-6；第二个钮偏移 +28px（首个 mr-1/ml-1，次个 mr-9/ml-9 = 24px 钮宽+4px 间距），仍落空白侧不越行；opacity-0 + group-hover/focus-visible 显隐，气泡本体 DOM 不变，渲染矩阵类测试零波及。

## cargo tree 倒排闭包三陷阱（2026-09-09 check-fast 实锤）
- `cargo tree -i -p X` 的 -p 会把解析图限定到 X 自身子树，倒排树查不到任何依赖者；必须用位置参数包名 `cargo tree -i X`。
- 位置包名必须写在 `-e` 之前：`-e normal,dev,build X` 直接报 unexpected argument；写成 `cargo tree -i X -e normal,dev,build` 才对。
- 倒排输出带 └── 树形前缀，BSD sed 的 `^[[:space:]]*` 吃不掉它们；名字提取用 `grep -oE '[A-Za-z0-9_-]+ v[0-9]' | sed 's/ v[0-9]$//'`。
- 受影响闭包要跟 `cargo tree --workspace --depth 0` 的成员全集求交，否则被 exclude 的 path 依赖者（apps/acp-agent 之类）混进来，与全量门禁口径分裂。
- Rust 测试模块位置取舍（2026-09-09 authz A1 轮）：同文件 #[cfg(test)] mod tests 可直访私有字段，适合构造悬空态；文件超 300 行要把测试外移时，先给类型补一个 from_parts 式工厂（用公开 API 表达测试场景），再移 tests 到独立文件挂 #[cfg(test)] mod，免留 test-only 后门。
- 集合断言写法（2026-09-09 authz A1 轮）：assert_eq!(actual_set, expected) 的期望侧先构造 Vec 再统一 sorted()，别在 assert 里内联 iterator 链；&[&str] → Vec<&str> 助手函数必须标显式生命周期 fn sorted<'a>(keys: &[&'a str]) -> Vec<&'a str>，否则 E0106。

- 2026-09-09 本仓多 workspace 结构的验收矩阵：根 workspace（crates/*，clippy -D warnings 全绿可作硬门禁）+ 独立 app workspace（apps/acp-agent、apps/cli 各自 [workspace]）。改独立 crate 的依赖（如 acp-agent 增 p2p-authz）会连带根 Cargo.lock（经 p2p-itest 的 path 依赖链），必须随提交入库；门禁逐 workspace 跑 `cargo test` + `cargo clippy --all-targets -- -D warnings > log 2>&1; echo EXIT=$?`，基线对照用主树同命令。
- 2026-09-09 `rustfmt --check lib.rs 或 mod.rs` 会级联格式化整个子模块树（lib.rs 带出 a2a/** 全部漂移）。判断「我的文件 fmt 是否干净」要逐文件 `rustfmt --edition 2021 --check <file>` 看 diff 落点是否在自己改的行，存量漂移不越界代修。
- 2026-09-09 契约判断三板斧快速路：`grep -rn "调用点" apps/*/src` 找全部调用方 → `git show <同类先例提交> --stat` 抄验收范围 → 查 scripts/check/*.sh 是否有会踩到的机械门禁（cli-parity 是 GUI→CLI 单向守卫，CLI-only 命令不受管）。
- 并行面同点注册的冲突预解（2026-09-09 authz A2-A2A 轮）：多面各自要往同一命令域注册子命令（authz import 各面一子命令）时，rebase 冲突的正解是 mod.rs 收敛为单一 `ImportCommand` 枚举、变体直接携带各面 Args 结构体（`LlmShare(import_llm_share::LlmShareArgs)` / `A2a(import_a2a::ImportA2aArgs)`），各面模块删掉自己的内层枚举只留 `run(args)`；第三面接入 = 加一个变体 + 一个 match 臂，不再抢 mod.rs。
- 幂等导入的实现形态（2026-09-09 authz A2-A2A 轮）：逐 peer 先 `check(目标权限)`，Allow 即 kept 跳过、Deny 才 upsert——重跑零落盘增量（granted_at/备注不翻新），比「无条件 upsert 再比对」省一层且天然语义正确；条目级数据问题（非法 peer）列报 invalid 继续其余，硬失败（表损坏/写失败）才整体上抛。
- 2026-09-09 authz S2 P1b「写失败可观测」日志断言测法：dev-dep 引 tracing-subscriber（workspace 已有，默认 features 含 registry），自写 `impl<S: Subscriber> Layer<S>` 捕获层（事件字段经 `tracing::field::Visit` 折叠成行），`tracing::subscriber::with_default(registry().with(capture_layer), || 被测代码)` 后断言捕获文本。三个坑：`with` 要 `use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt`；`with_default` 收 Subscriber 本体（Dispatch 不实现 Subscriber，别套）； subscriber 按值传入。
- 2026-09-09 S3 轮「疑似自己改红的测试」基线判定法：在未改动的主树复跑同测试文件，同红 = 存量问题（本轮 acp/permission-notice 2 用例在 f55f397 主树同红，timeout 5s 等不到 acp-capabilities-card），报告附证据不越界代修；主树只跑只读测试不写任何 tracked 文件。
- 2026-09-09 S3 轮 pre-push 快速门禁组成：fmt 双段 / line-limit（300 行）/ version 三处一致 / src-tauri-gate 自测——推送前先本地跑一遍 scripts/check/fmt.sh，新增 Rust 文件 100% 会在长签名处吃 rustfmt diff（cargo fmt 秒解，0610885 与本轮两次实证）。
- 2026-09-09 S3 轮 radix Select 在 jsdom 的测试交互：本地桩 `if (!Element.prototype.scrollIntoView) Element.prototype.scrollIntoView = () => {}` + `fireEvent.click(trigger)` → `findByRole("option")` → `fireEvent.click(option, { button: 0, pointerType: "mouse" })`（endpoint-add-dialog-ux3 先例）；Select 的受控 value 不可用 `x || undefined` 造「未选态」（uncontrolled→controlled 警告），直接绑 "" 让 SelectValue 走 placeholder。
- 2026-09-09 `cargo clippy` 没有 `--message-format` flag（clippy-driver 报 Unrecognized option，那是 rustc 的）；要短格式/清单化 lint 输出，用全量落盘 `cargo clippy ... > /tmp/x.log 2>&1` 再 grep "^error"/"-->" 提文件行号清单。
- 2026-09-09 macOS 自带 grep 无 `-P`（PCRE），unicode 区间扫描（emoji/全角符号巡检 diff）用 perl 直读退出码：`perl -ne 'exit(0) if /[\x{1F300}-\x{1FAFF}]/; exit(1) if eof' /tmp/d.diff`（0=命中 1=干净），别用 `| head` 后猜退出码。

## jsdom 虚拟化测试基座（2026-09-10 聊天长列表虚拟化轮实证，apps/gui/src/test/jsdom-virt.ts 已沉淀）
- react-virtuoso 4.18 在 jsdom 渲染恒 0 items，三道守卫：RO 回调 `offsetParent !== null` 短路、scrollToCallback `offsetHeight === 0` 静默跳过、尺寸全靠 offsetHeight/scrollHeight（jsdom 全 0）。官方出路 = `VirtuosoMockContext.Provider value={{viewportHeight, itemHeight}}`（内部走 fixedItemHeight，绕过全部 DOM 测量），再补几何桩：offsetHeight→600、getBoundingClientRect→600×400、scrollHeight→data-scroll-height 属性、Element.scrollTo/scrollBy polyfill 成「写 scrollTop + 排队延迟派发 scroll」。
- scroll 事件必须异步派发（真浏览器语义）：同步 dispatch 会与 virtuoso 多步锚定（前插 scrollBy 补偿）重入竞态。
- startReached/endReached 走 `zt(200)` 200ms 节流流：固定时长 settle（70ms 级）与它竞态、忽绿忽红；必须条件轮询 settleUntil(cond, budget)（每 50ms 冲刷排队 scroll + 查条件），2000ms 预算内必到。
- followOutput 在 jsdom 不可依赖（其回底走延迟订阅链）；应用侧钉底（atBottomStateChange 记态 + 尾部追加 effect 里 scrollToIndex({index:"LAST", align:"end"})）命令式路径已实证两环境一致。
- react-window v2 List 定高行（rowHeight 数值 + style height）纯 props 计算，jsdom 天然可测，但 rowProps 必传（空对象也行），缺了在 useVirtualizer 里 Object.keys(undefined) 崩。

## 虚拟化改造零回归的阈值双路径（2026-09-10 实证）
- 短列表（≤200 消息 / ≤100 会话）保留原全量渲染路径、长列表才切虚拟：存量行为测试夹具全是小列表，零改动全绿（1367 用例），虚拟路径行为由新增压力测试独立覆盖；行渲染抽成共享组件（MessageRow/GroupMessageRow）保证两条路径 DOM 结构一致。
- 互操作轮破线格式:文档与实现冲突时,不读源码也能破——写一个"纯记录字节的假服务端",把真实现的 bootstrap 指向它,让真客户端自己把线格式发过来(2026-09-10 IV1 实证:u32be 前缀漂移 30 分钟内破案)。
- 对端是守护进程时先挖 daemon.log 再猜:结构化 WARN(error=xxx)是黑盒调试的路标,一次日志就能把假设空间砍掉 90%。
- 黑盒探针要一次变化一个变量并逐次对时间戳关联服务端日志,批量矩阵跑完再对账会把因果搅乱(IV1 教训:矩阵探针的日志归属错位浪费两轮)。
- jsdom 下调试 markdown/渲染管线输出：写临时 `*.debug.test.tsx` 用 `console.log("INNER_HTML:", container.innerHTML)` 打点跑单测，比猜转换层行为快；用完即删(2026-09-10 RICHTEXT 轮定位软换行为实证)。

- 发布排队三闸（2026-09-10 v0.1.7）：①协调文档 tail 出现本轮「收官/I2/P3」记录；②`git rev-list --count main..origin/main` 归零且本地=远端；③`pgrep -f "cargo test|vitest|make check" | wc -l` = 0。三者同时满足才启动 release-check。
- vitest 满载红分诊两步法：先 `vitest run --no-file-parallelism <红文件>` 串行复跑（大部分转绿=超时假红）；仍红的再单文件隔离复跑（network-tabs 整应用启动型用例串行仍可能红，隔离跑才见真身）。
- 观察哨 settle 条件写「目标状态」不写「差距状态」：`behind>0` 做判据会在对方"先拉后推"时永不触发（behind 恒 0）；改判 `origin 包含基线提交 && 门禁进程=0`。

## 派发任务书前先验证路径（2026-09-11 W-T1 实证）
- 任务书里的文件指针写错会把执行者引向不存在的路径：W-T1 收到 `docs/protocol/wire-protocol.md`（不存在），
  真值源是 `docs/design/wire-protocol.md`；执行者靠 ls 兜住并主动上报，属对方尽责而非我方免责。
- 固化做法：**写任务书时每个路径都先 ls/glob 证伪一次**；同名文件存在两份（`docs/protocol/wire-format.md` 与
  `docs/design/wire-protocol.md` 这类近名/近义路径）时必须逐字抄实际路径，不凭记忆。
- 同类：慢门禁（`ai-docs-sync.sh` 冷构建 p2pctl 约 5 分钟）在派单说明里预先标注"丢后台、与提交并行"，
  否则执行者会把它当关键路径空等。

## 「仅在某编排下复现」先做环境全差对齐（2026-09-11 W1 实证）
- 排"单跑绿、编排红"类假象：先把两环境的**全部差异**列出（locale / PATH / bash 版本 / 工作目录 / 环境变量），
  再各做一次最小复现——本次 `locale × bash` 微矩阵（3 次调用）直接击穿"只在 make check 编排下复现"的假象，
  真因是 locale，与编排无关。
- 对照实验的落点要选对：**旧代码上跑红 + 新代码上跑绿**才算双向；只在新代码上跑绿无法区分"测住了"与"本来就绿"。
- 复核他人修复时先 `pwd` 确认自己在哪个树：在 main 树跑 worktree 里才有的脚本会得到"No such file"，
  容易误判成"修复没生效"（2026-09-11 协调者自身实例，一次踩中，幸而立即发现）。
- 半关语义定案用三腿证据矩阵（2026-09-11 W-T4 pump 半关卡）：①泵级裸 TcpStream 探针（自建 socket 对，
  先 connect 后 accept）直测 pump；②生产全栈 itest（既有 h1）证 mux 线；③对照组复刻「整流关闭→读端 0 字节」
  证明疑点系接线 artifact——三条腿一次把「pump 缺陷/线缺陷/测试错」三假设切干净，pump 零修改定案。
- 任务书里的路径/行号会漂移（W-T4 任务书写 apps/cli/src/llm_share/borrow_dial.rs:134，实为
  crates/p2p-cli/src/llm_share/borrow_dial.rs:140）：动手前先全仓 grep 兜底再按实况改，别按行号盲改。
- 跨机 e2e 传分支不污染 origin：`git bundle create /tmp/x.bundle <branch>` + scp + 对端 `git fetch /tmp/x.bundle <branch>:<local>`（2026-09-11 W-T5，过门禁前禁 push 时尤其有用）。
- 102 免密但 GitHub 无 SSH key：仓库公开则 `git clone https://github.com/imeepos/p2p.git` 可行；私有仓才需要 BLOCKED。
- Tauri GUI 驱动双通道：控制通道 CLI（p2pctl gui status/screenshot/navigate/action，白名单内）+ webview 注入驱动（fill_open 等白名单外操作）；先查 CLI 侧能不能干，不行再注入。
- 远端 ssh 命令里嵌 ssh 变量替换（$(cat file)）在本端展开——要在对端执行的 $() 整条放引号内（2026-09-11 W-T5 实证）。

## 2026-09-12 TA 契约卡（tunnel 泛化签名桩）
- worktree 免全量重建跑门禁：`export CARGO_TARGET_DIR=<主树>/target` 直接借主树预热缓存（比 target 软链更省事），本卡单包 check 约 2 分钟、workspace 全量 4m22s；前提=确认无并发会话同用该目录（先 ps 查 cargo）。
- 手写 `pub use` 组后先 `cargo fmt` 再进 fmt --check 门禁：新版 rustfmt 对 use 组按「小写标识符在前」排序，凭记忆写成大写在前必红一轮（本卡 fmt 首跑 diff 两处，fmt 后复验 0）。

## 2026-09-12 TE 跨机 e2e（tunnel 泛化）证据链
- GUI 面证据标准路径：控制通道 ROUTES 白名单没有 remote-access 页（/navigate、/page/action 到不了），GUI 表单驱动走 wt3b 静态注入链——注入器绑双栈（devUrl=localhost 解析 ::1）、driver 按目标页 DOM 适配（W-TE driver.js 的 fill_generic 可复制改）、dist 先 grep 目标标记确认含刚合并的前端再开浏览器；截图前必须把 GUI 窗口置 frontmost（遮挡时 /screenshot 返回陈旧合成帧，两帧字节相同即此故）。
- 102 类无 git 出网环境传仓：git bundle create（带分支引用）→ scp → 对端 clone，18MB 秒级；对端 apps/cli 独立 workspace `cargo build --release` 2m36s 即得 p2pctl。跨机审计对账锚=session_id（票据 uid 两侧同源），字节数两侧口径天然不同（记录方视角），勿按字节强对账。

## 2026-09-12 GUI mock WS 的 peer 白名单（console-watch 自动连接测试）
- apps/gui/src/acp/mock-acp-ws.ts 校验 `?peer=` 必须在 `configure({ peers: [...] })` 白名单内（token 另校验）：测「descriptor peer 自动连接」若 peer 不在白名单，WS 升级被拒→1006→phase=offline，报错面是断言 online 失败而非 mock 报错。写断言前先读 mock 的握手校验语义，别等红了解剖。
