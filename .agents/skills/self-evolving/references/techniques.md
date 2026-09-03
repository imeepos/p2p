# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

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
