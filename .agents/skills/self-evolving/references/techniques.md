# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

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
- 2026-09-05 ACP3：并行会话会 prune/打断彼此的 worktree——worktree add 用 run_in_background 跑（前台 ~60s 超时会杀掉 801 文件的 checkout 留半成品，本日两次实证），建成后立即 git worktree lock --reason 自保；每次操作后 git -C <wt> rev-parse --show-toplevel 核对没回落主树（输出主树路径=管理区被清的事故信号）。
- 2026-09-05 ACP3：run_code 的 tools.write 内容经 JSON+JS 双层转义，Rust 转义序列写 \n 实际落盘真实换行（模板串里单反斜杠就是换行），字符串字面量当场编译红——写完必须 grep 查字符串内断行；修复用 python3 精确 replace（py 里 \\n 才是字面反斜杠 n），比 edit 锚点匹配可靠。
- 2026-09-05 ACP3：长流双向泵（WS⇄yamux）挂死自愈配方——spawn 双任务 + select 首侧结束 abort 另一侧（join 双侧都 pending 就永不结束）+ 写 16KiB 块/5s 超时重 kick + 读 30s 超时重 kick + PeerDisconnected 事件竞速兜底；yamux 批量窗口更新（<半窗不发）遇 echo 停等流量会饿死写侧（known-issues 同日条）。
- 2026-09-05 移动分组修复：Radix Select 在 jsdom 的可测配方——`fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: "mouse" })` 开下拉（react-select 源码硬性检查 `pointerType === "mouse"`，缺省空串直接不开），option 用 `findByRole("option", { name })` 拿到后 `pointerUp`+`click` 各一次；前置桩 scrollIntoView/hasPointerCapture/releasePointerCapture 三件套。先探针 `node -e` 验证 jsdom 有 PointerEvent 再写测试，省一轮盲调。
- 2026-09-05 移动分组修复：收尾 push 判活别信 `cmd | tail; echo $?`——那是 tail 的退出码；开头 `set -o pipefail` 或 `${PIPESTATUS[0]}`（ff5831e 已沉淀过此课，本次仍复犯，验证脚本要模板化）。


## 2026-09-04 T22 门禁脚本环境加固轮

- run_code 里 tools.write 整写 bash 脚本（TS 模板串 + 反斜杠花括号转义）后必须 read 回读 + bash -n + 真跑三连验证：本次回读抓出两处静态读不出的 bug——local 临时变量被 EXIT trap 引用（trap 触发时函数已返回、local 已出作用域，set -u 下 unbound 污染退出码；trap 要引用的临时路径改存全局变量）与变量改名漏网的引用行（fakebin 仍指旧名）。
- bash 自测夹具伪造"可执行二进制"：touch 建的文件 644 不可执行，被测脚本里 [ -x ] 前置判定会直接走"缺失需重建"分支（本次 self-test 场景连红实锤）——夹具建文件后补 chmod +x。
- 抽共享 lib-*.sh 前先查门禁自测夹具的复制面：tests/cli-parity.sh 只 cp 单脚本进临时夹具树，主脚本 source lib-*.sh 在夹具里必然缺文件（set -e 即死），而夹具测试文件常在红线禁改清单——加固逻辑选择脚本内联自包含，重复 ~40 行换夹具兼容。
- 并行会话密集推进 main 时 ff-only 前置三查：git fetch 后核对（1）本地 main == origin/main；（2）merge-base --is-ancestor main <合并点>；（3）主树 status 干净。本轮 main 在会话中段从 895c3a4 前进到 544b1ae，三查全过才 ff。
- worktree 新 checkout 跑 make check 前先补环境：apps/gui/node_modules 不进 git，gui-check 的 eslint 直接 command not found（环境假红非代码红，归因先看报错形态）；pnpm install 后 ls node_modules/.bin/eslint 确认再重跑。cargo 侧 worktree 首跑必全量编译，先后台预热 cargo build 再跑验收脚本。
- 脚本双 bash 兼容验证法：/bin/bash -n + /opt/homebrew/bin/bash -n 各过一遍语法，--self-test 两边各跑一遍（验收链用哪个 bash 取决于调用方 PATH，不能只验一个）；变量展开一律花括号化（延续 90e062a 对 bash 5.3 多字节相邻展开的防御）。
