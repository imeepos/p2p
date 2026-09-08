# Self-Evolving Notes

## 2026-09-08 通讯录去好友分组（feat/contacts-flat-friends）

- 哪个坑浪费了最多时间？
  基本零返工。唯一一轮门禁红是 IPC 调用点守卫：移动分组对话框是 chatFriendUpdate 的唯一界面调用点，删组件后守卫测试立即红。豁免清单就是为「后端有能力、界面无入口」设计的登记制，写明原因（契约保留供 CLI friends update --group）即收敛。
- skill 有没有提前警告我？
  命中多条：lessons 23/53/134 的「worktree 路径先读后改」防住了 write 工具同款拦截；IM-T42 调用点守卫条目让我删组件前就预期到守卫会红；反向同步 fetch 在 ff-only 前真的捡到了并行推进的 main（694c76f），照既有流程一次合并成功。
- 重来一次会怎么做？
  移除型任务开工先 grep「目标功能是哪些 IPC 方法的唯一调用点」，把守卫豁免登记排进改动清单，而不是等门禁红了再补；本次即先跑全量 contacts 测试再收尾，红点一轮收敛。


## 2026-09-08 聊天附件下载改保存对话框 + 后台导出进度（feat/chat-media-export）

- 哪个坑浪费了最多时间？
  业务实现一次过；时间花在三处门禁摩擦：① main 上 clippy 1.98.1 新 lint items_after_test_module 本就红，先误以为是自己引入；② hardcoded-copy 门禁连行尾中文注释都拦；③ toastSuccess 封装签名与 sonner 原生不同。三处均有明确报错，单轮修复。
- skill 有没有提前警告我？
  部分：lessons 有「管道命令 exit code 被 tail 吞」「run_code 模板串转义」直接避坑（本次媒体下载组件含 `${}` 内嵌的 TSX 改用普通串拼接一次写成）；没有覆盖「基线门禁自身红」与「本仓封装 API 签名漂移」，已喂回 lessons。
- 重来一次会怎么做？
  worktree 建好第一时间并行跑 pnpm install + 基线 clippy，把门禁存量红前置暴露；Tauri 命令核心逻辑收 impl Fn 回调参数脱离 runtime 直测（本次 export_media 一次成型，值得沿用）。

## 2026-09-07 rail 多主题适配 + 删除重复设置入口（fix/rail-theme-and-avatar）

- 哪个坑浪费了最多时间？
  不在业务改动本身（半小时内完成），而在收尾：收尾四步链成一条 bash，输出截断 exit=null，push --delete 和 push main 实际没执行，远端残留 fix 分支；靠 ls-remote 计数核验才补删。另有一个 run_code 正则经 JSON 转义跨行的 SyntaxError（报误导性的 "Unmatched )"），换 edit 工具传字面量一次过。
- skill 有没有提前警告我？
  有：「并行会话基线」让我开工前 fetch 核对 main==origin/main；「误闯并行 worktree」让我开局 git worktree list 识别两个他人半成品 worktree 并避开。收尾核验粒度（每步独立+机械核验）是本轮新坑，已喂回 red-lines/lessons。
- 重来一次会怎么做？
  收尾四步从第一条命令起就逐步独立执行带核验；给 worktree 装依赖直接给足 timeoutMs（本轮 2m20s 装完）；样式类「固定色绕过主题系统」类缺陷先找语义令牌再动手。

## 2026-09-04 gui-updater 轮：应用内下载安装更新（updater 插件全链路）

- 哪个坑浪费了最多时间？
  两个都指向同一根因：ext512 外置卷小文件 I/O 病态慢。worktree add / rm -rf node_modules / clone 全被 60s 默认超时静默杀掉，留下「命令成功但输出为空」与「目录半成品」两种假象，排查烧掉近 30 分钟才定位到卷性能；之后把主战场搬到 /tmp clone（8m48s clone 换来全程秒级操作）立刻顺了。
- skill 有没有提前警告我？
  部分：known-issues 已有「bash 60s 超时杀进程」现象条目（今日他轮已补），但没有根因与对策；「并行会话活跃要逐步验证 git 状态」已有，照做后避免了在残骸 worktree 上继续干活。本轮补上根因条目与 clone-to-/tmp 工作法。
- 重来一次会怎么做？
  在这台机器上：任何涉及大量小文件的 git/fs 操作开局就显式 timeoutMs ≥ 600000 或直接 background；功能开发默认 /tmp clone 战场而不是 worktree（自持 .git 还免疫并行会话的 git 手术）；「静默空输出」一律先跑验证命令再继续，绝不假设成功。

## 2026-09-04 RS 排障轮：session_report 缺注册 + rendezvous 掐线/客户端卡死

- 哪个坑浪费了最多时间？
  两个：① harness bash workdir 参数失效，测试跑在主树得出「102 passed 全绿」假象，直到 clippy 输出出现主树路径才警觉，重跑才见真测试；② 跨进程 rendezvous 排障在黑盒里绕了很久（bootstrap/loopback/LAN IP 换了三轮），后来才意识到该一步到位写跨节点 itest 拿确定性复现——双侧日志时间线（probe missed 3 次后 closing → link ended）其实早给出方向。
- skill 有没有提前警告我？
  部分：lessons 里已有「陈旧二进制假绿」「run_code 字符串转义」同类目，但没覆盖 workdir 失效与 heredoc 美元花括号插值两个新变体（本轮已喂回 known-issues/techniques）；「先写复现测试再修」是本仓既有纪律，照做后一次定位成功。
- 重来一次会怎么做？
  跨进程类缺陷直接按「双侧 debug 日志 + 最小复现 itest」开局，不做三轮环境变量实验；每个测试结论落地前先核对 pwd 与分支归属；多进程 lab 的冒烟脚本首轮就控制 EOF 时序（pump 排空即退是设计行为）。

## 2026-09-03 发布门禁事故复盘（feat/release-gates）

- 哪个坑浪费了最多时间？
  不是修断言本身，而是确认 CI 实际覆盖面：GitHub workflow 只在 PR/tag 跑，Gitea workflow 虽写了但当前仓库没有 gitea remote，因此 bump 直推 main 完全没有门禁。
- skill 有没有提前警告我？
  已有经验提醒不能只跑 cargo、GUI 必须 gui-check，也提醒远端名要实测；但没有把“CI 文件存在不等于实际触发”和“门禁脚本自身必须被门禁”固化。此次补入 known-issues/lessons。
- 重来一次会怎么做？
  开始先画提交→触发器→required check→tag 的路径矩阵，逐个确认实际远端和事件覆盖；任何新门禁同时写成功/失败夹具并纳入 make check，避免门禁代码自身成为未测试盲区。

## 2026-09-03 GUI 节点行内拨号/挂断（feat/peer-dial-hangup）

- 哪个坑浪费了最多时间？
  提交拆分返工：预暂存文件混进第一个 commit（git add <paths> 前没查 status 暂存列），后续 commit 因 nothing to commit 静默跳过，log 复核才发现两个 commit 装错内容，reset 重拆一遍。另有 edit 按路径记账的旧坑再踩（worktree 副本未先 read）。
- skill 有没有提前警告我？
  部分：line-limit 教训让我直接把 disconnect 放进独立 hangup.rs，一次过；mock 语义对齐与 commit 拆分复核是新坑（已喂回 lessons.md）。
- 重来一次会怎么做？
  每个 commit 后立即 git log --oneline 复核数量与归属；开工时对 worktree 内所有待编辑文件先批量 read 再动手。

## 2026-09-02 V 文档整理（docs/organize）

- 哪个坑浪费了最多时间？
  run_code 多行 bash 字符串的 JS 语法错误，一次失败一次重试；改用数组拼接后一次过。
- skill 有没有提前警告我？
  没有。skill 此前只有 Rust 生态教训，没有 run_code 字符串转义类教训（已喂回 lessons.md）。
  另外设计稿与代码不一致（PeerId 推导）靠任务提示才去核对，应默认不信任设计稿（已喂回）。
- 重来一次会怎么做？
  开工前先 glob "crates/**/*.rs" 拿全清单再排阅读顺序，中途不会撞 rendezvous/ 子目录缺失。

## 2026-09-02 X 构建门禁（chore/ci-gate）

- 哪个坑浪费了最多时间？
  fmt 门禁红引发的连锁：授权 fmt 归一后拆行把两个文件顶过 300 行红线，二次裁决提问超时（操作者 10 分钟未响应），按既有授权精神以独立可 revert 提交继续；另有自己"先覆写后抽段"的顺序错误导致重做一轮。
- skill 有没有提前警告我？
  部分命中：Cargo.lock 工具链漂移、rebase 前别信旧扫描两条 lessons 都提前避坑。没预警的：bash test = 不做通配匹配（已喂回 known-issues）、fmt 拆行会推高行数（已喂回）。
- 重来一次会怎么做？
  接手先跑一遍全部检查摸清存量状态再写门禁脚本；fmt 归一提交后立即复查行数红线，把超线文件拆分纳入同一计划，而不是等门禁红了再补救。

## 2026-09-02 E4 discovery 稳定性派单（p2p-D-E4）
- 哪个坑浪费了最多时间？
  两处小坑各耗一轮重编译：tracing event! 动态级别编译错（换成字面量分支）、cargo fmt 后 edit 报文件已变（重读解决）。mdns.rs 顶到 300 行红线是一次性算行数后主动压注释腾余量，没红。
- skill 有没有提前警告我？
  命中三条：多行 commit message 用 -F /tmp 文件、收尾前重新 fetch（main 中段真的进了 1b997f9）、主树 ff-only 前核对 cwd 与 merge-base——收尾一次通过。
- 重来一次会怎么做？
  动手前先跑 cargo test 摸清存量测试基线与 dev-deps（本次读 Cargo.toml 才发现没有日志捕获设施，改走纯逻辑状态机断言）；写代码前把 300/60 行红线余量算好再落笔。

## 2026-09-03 p2p-console GUI 10 小时协调战复盘（协调会话）

### 做对了的
- 契约先行冻结（gui-contract.md），A/B 对同一契约并行编程零等待；缺口走"报协调者→加法修订"闭环，两次修订（tsMs/云端端点）都干净落地。
- 文件所有权零交集切分（A=src-tauri，B=前端除 src-tauri，C/D 各自 views 子目录），四会话并行从未撞文件。
- 每单机械验收命令先行写进派单书，验收只看命令输出，不看故事。

### 踩坑与修正
1. ff 失败后用分号续链误删未合并分支 ×2（G-E、G-C2）：凭回报中的 tip 哈希完整恢复。教训已入 red-lines（合并尝试必须 && 短路）。
2. 轮询提醒链叠加成 7 条导致检查过频：任意时刻只允许存在一条链，触发后先删自身再续。
3. 派单书没写"skill 经验提交放 feature 分支"，两个会话把笔记提交到主树 main 挡了 ff 线性（48d170b/e8cb4cb）。
4. 系统休眠导致 wall-clock 与运行时长脱节：窗口口径改按运行时长累计，唤醒后先全量盘点再动作。

### 数字
- 23:44 派单 → 06:35 全合并 → 10:10 打包产物，合并 11 个 feature 分支、约 60+ 提交、i18n 287 键双语、Rust 38+smoke 测试、前端 30 测试。


## 2026-09-03 白屏事故复盘
- 哪个坑浪费最多时间：不是修 bug，是「没人知道坏了」——门禁盲区让故障从引入到人肉发现隔了一整晚。
- skill 有没有预警：没有。此前条目全是执行层纪律（分号链/分支），缺「验收必须覆盖用户真实路径（应用能启动）」这条，本次已补进 red-lines。
- 重来一次怎么做：GUI 任务的验收命令写死「启动冒烟绿」，而不是让人打开窗口当测试员。
## 2026-09-03 G-H 观测单（用户点名：前端报错感知不到、没给自己留操作入口）
- 哪个坑浪费最多时间：clippy 失败被 `| tail` 掩成 exit 0 走了一轮假绿——管道吞码已是 red-lines 条目仍踩，门禁命令一律 `> log 2>&1; ec=$?` 显式收码。
- skill 有没有预警：部分命中（edit 按路径记账、worktree 先 pnpm install、session_link_list 无参绑定失败绕行都有条目）；没预警的新坑：run_code 模板字符串吃 markdown 反引号（已喂回 red-lines）。
- 重来一次怎么做：开工先 session_link_list 摸并行会话归属，binding 失败就用 git 证据 + talk 兜底对齐；本次最有价值交付是把「感知→定位→修复→复验归零」做成闭环——gui-agent 上线首跑即实证 selectPeerList 无限重渲染与 Button ref 噪音两个存量缺陷，修复后 errors 三通道全空即机械证明。


## 2026-09-03 G-U1 在线更新检查桥接（feat/gui-update-check）
- 哪个坑浪费了最多时间？
  reqwest 0.13 feature 改名（rustls-tls 已不存在）浪费一轮完整编译；另 red-lines 已有的"管道吞 exit code"仍重踩（clippy 失败被 | tail 报成 exit 0），靠读输出文本兜住，下次门禁命令一律收 PIPESTATUS。
- skill 有没有提前警告我？
  命中：worktree 建分支、契约逐字对齐、依赖前查 Cargo.lock 复用图内包（url/chrono 零新增成本）、main 高速推进先 fetch 再定基线。没预警：reqwest feature 改名（已喂回 lessons.md）。
- 重来一次会怎么做？
  开工先跑一遍目标依赖的 cargo add --dry-run 或查 docs.rs features 表；合并-门禁做成可重复循环，用 is-ancestor 机械收敛。

## 2026-09-03 G-U2 更新提醒前端（feat/gui-update-remind）
- 哪个坑浪费了最多时间？
  两处都是测试侧：vi.mock 工厂引用顶层 vi.fn() 的 TDZ（vi.hoisted 才对）；zustand setState 不包 act() 导致 effect 异步冲刷、断言读旧值，加上夹具 helper 顺手重置去重标记把被测逻辑破坏——4 个失败全在测试自身而非实现。
- skill 有没有提前警告我？
  命中：locale 先行小提交、mock-ipc 行数余量自查（297/300 惊险守住）、react-refresh 纯组件导出红线提前把 helper 拆文件。没预警：edit 工具巨量回显伪影差点误判文件损坏（已喂回 known-issues：大文件编辑后 git diff 权威核验）。
- 重来一次会怎么做？
  zustand 组件测试从第一个用例就统一 act() 包 setState；写夹具 helper 时把「哪些字段是断言目标」列清，禁止 helper 隐式重置被测状态；协调者 talk 巡检正好像心跳，主动在关键节点（locale 落盘/验收全绿）回报一次省得被动等查。

## 2026-09-03 relay 页 FormProvider 白屏（用户打开即报错，fix/relay-form-provider-crash）
- 哪个坑浪费最多时间？
  修 bug 本身 5 分钟；真正耗时是向用户论证「为什么 95 个测试全绿还会白屏」——测试盲区有三层：启动冒烟只踩默认路由、relay 目录零渲染测试、notice 组件只被 settings 组合的测试覆盖。红→绿双向证明（stash 修复跑新冒烟确认必红）补齐了证据链。
- skill 有没有提前警告我？
  部分。red-lines 已有「GUI 必须 gui-check 绿」与白屏/可观测条目，都是执行层纪律；没预警「context-provider 按组合失效」这一类（本次已喂回 known-issues/lessons/red-lines 三处）。另 red-lines 的「管道吞 exit code」仍重踩一次（gui.sh 在主树跑假红一轮），workdir 与门禁落点对齐后收敛。
- 重来一次会怎么做？
  新页面复用带 context 依赖的组件时，同步写「按真实组合挂载」的组件测试再动手改；冒烟测试天生于全路由循环而非入口路由；跑门禁先核对 workdir 与目标树一致。

## 2026-09-03 拨通即闪断（用户节点列表点拨号报修，fix/dial-flash-disconnect）
- 哪个坑浪费最多时间？
  两处：①诊断测试的 echo handler 只注册在单侧，b→a 的 early eof 让我误判为连接分家的直接证据，绕了一圈；②双向拨号 pre-fix 的失败呈现依赖 interleave，回归测试在 pre-fix 代码上偶发假绿，只能 revert 修复做红→绿证明才敢下结论。
- skill 有没有提前警告我？
  命中：worktree 分支、红→绿双向证明（lessons 92）、管道退出码、分号链禁令。没预警：「测试只断言第一个成功事件」这一连接类测试盲区（本次已喂回 lessons）。
- 重来一次会怎么做？
  先写「观察窗零断开 + 后续往返可用」的连接活性测试再动代码；诊断 helper 先自证双向可用；回答「为什么测试没覆盖」时直接摆现有用例的断言终点（全部止步 PeerConnected），比口头反思有力。## 2026-09-04 relay 重连风暴日志分析（用户贴运行日志求诊断，纯分析轮）
- 哪个坑浪费最多时间：日志三种现象并存（peer 30s 循环、relay 10.5s 循环、直连全灭），容易先入为主去查探测协议；真正突破口是机械时间指纹——uptime 恒 30.00x s、控制流关闭恰在 connected 后 +10.008s，把现象钉到定时器网格而非网络抖动。
- skill 有没有预警：known-issues 已有 keepalive/tcp 条目，但没预警「客户端周期与服务端超时窗口同值贴线竞速」与「拨号 socket 协议族和地址簿地址族不对齐」这两类（本次已喂回）；周期性日志先算相邻差分找恒定周期（sigma<5ms 即定时器）这一排查手法已补进本条。
- 重来一次怎么做：拿到周期性日志先做时间差分表，恒定周期必对应代码常量，grep Duration 常量对表；凡 A 端周期性动作对 B 端超时窗口，必问谁先谁后、输了会怎样。

## 2026-09-04 诊断真实日志与一键清理

- 哪个坑浪费最多时间：委派任务超时后已写入提交但仍有未完成 UI，且翻译键重复导致全量测试失败；连续 edit 也因文件被前序工具修改而触发 stale-read 保护。
- skill 有没有提前警告：已有先读后改、提交复核与失败门禁经验，但没有提醒委派超时后先完整审计 diff，也没有提醒批量插入 locale 键前先 grep 防重复。
- 重来一次会怎么做：委派返回超时后立即以 git diff/status 和关键文件 read 盘点；每次 locale 修改前 grep 目标键；全量测试失败必须修复后再提交并再次跑完整门禁。

## 2026-09-04 五连修执行轮（relay 保活/QUIC 双栈/探活判死/地址簿卫生/日志降噪）
- 哪个坑浪费最多时间：fmt 门禁两轮红——第一轮只 fmt 改动的三个 crate，第二轮才用 --all 收口；expect_err 条目 known-issues 已有仍踩进去（SecureConn 无 Debug），凭惯性手滑，说明条目必须写到「替代写法」层级才防得住（本次已补 match 取 Err 臂写法在案）。
- skill 有没有预警：worktree 按路径读缓存条目防住了（先读后改一次过）；expect_err 有条目没防住；新坑 quinn Endpoint::new 收 std socket 与 v4-mapped 令 is_unspecified 失真已喂回 known-issues。
- 重来一次怎么做：改代码批次收尾一律 cargo fmt --all 再提交，不等 fmt-check 抓；五任务串行时每个任务一气呵成 走 worktree（建/改/测/提交/合并/清）模板化 bash，实测单任务 3-4 分钟。
## 2026-09-04 遗留事项消灭轮（relay 重部署 + 串址勘误 + mDNS 全地址解码）
- 哪个坑浪费最多时间：上一轮把「共享 IP+异端口」误判成串址并写进了 known-issues，本轮消灭遗留事项时复核日志才发现端口不同——误判已按 append-only 勘误条目纠正。多花的时间在补勘误与重验，比当初多比对一眼端口划算。
- skill 有没有预警：没有。「下结论前比对完整键（含端口）」这类证据纪律条目已随勘误条目落地。
- 重来一次怎么做：凡是「两条记录共享 X」类指控，先把 X 的完整字段摆出来再定性；部署类动作（deploy 脚本）直接用仓内既有脚本，先读脚本确认幂等性与凭据来源再执行。

## 2026-09-04 会话清理第二轮（47 归档，100 槽位）
- 哪个坑浪费最多时间：没先 grep references 就开查。上轮已写明数据根是 `~/.dsh/dsh012-clean/`、管理视图返回全量，我只加载 SKILL.md 就动手，拿 `~/.dsh/storages/workspace.json`（另一实例）对账，中间结论「47 个归档全部未生效」是错的，白走 4 步探查（grep 源码、找存储后端、ps 查进程）才由 13≠17 的 workspaceIds 数对不上纠正。
- skill 有没有预警：有，techniques.md「2026-09-04 DSH 会话记录清理」整节就是答案；暴露的加载面问题——SKILL.md 只载流程，references 要自己读。已把「开工先 grep references 关键词」喂回 techniques。
- 重来一次怎么做：同类运维任务先三查——references 关键词、`ps aux | grep bin.js` 定实例、落盘 JSON 定数据根；工具调用「成功」自报一律不算数，验收只认持久化状态比对。

## 2026-09-04 env worktree 自动接入（方案3 post-checkout 钩子）

- 哪个坑浪费了最多时间？
  相对 core.hooksPath 在 worktree add 时相对发起 cwd 解析、钩子静默跳过：第一次验证无任何输出也无 .env，靠对照实验（从有 githooks/ 的目录发起就触发）才定位。真正的坑是"git 对不存在的钩子文件静默跳过"。
- skill 有没有提前警告我？
  没有。skill 有 worktree 协作教训但没有 core.hooksPath 路径解析语义（已喂回 known-issues）。
- 重来一次会怎么做？
  写钩子方案第一步先放 echo 哨兵钩子，实测"是否触发、从哪个 cwd 解析"，把路径语义定清再写业务逻辑，省一轮对照实验。

## 2026-09-04 IM-T50 反思

- 哪个坑浪费了最多时间？
  主树全量验收在并行负载下反复假红（repair-bridge CARGO_BIN_EXE 竞态 + vitest 超时雪崩），三轮重跑被并行会话的 cargo 大任务反复打断；若第一轮就做隔离定性+错峰策略，能省两轮。
- skill 有没有提前警告我？
  部分有：IM-T45 端口互踩条已给「单测复跑定性」手法，但没覆盖 CARGO_BIN_EXE 二进制竞态与 vitest 负载超时变种（已补）；zustand `?? []` 坑已有登记但未在写测试时回忆，检索时机要前移到「写涉及 store 的代码前」。
- 重来一次会怎么做？
  验收放在 worktree 全绿后先与并行会话错峰（看 load 再跑主树全量）；协调通信每完成一个里程碑主动发一条简报，不等通牒。

## 2026-09-04 GC3 页面语义协议轮反思
- 哪个坑浪费了最多时间？
  dispatch_task 带「冷 cargo 全量构建」的任务必然撞 600s 墙钟（~25 分钟沙箱构建 + 甄别半成品 + 重派成本）；接管的甄别本身很快（git status + 逐文件 read），但之前的 30 分钟构建是纯浪费——构建与代码产出应当从一开始就拆开。
- skill 有没有提前警告我？
  没有任何 dispatch 墙钟/接管条目（用户当日已把「接管前先查产物」固化进 AGENTS.md，references 侧是空白，已补）；worktree add 中断三态脏局面也无登记（已补）。
- 重来一次会怎么做？
  派发前把任务切成「无构建的纯编辑」+「后台构建验证」两段；worktree add 独立成步给足超时；前端动作盘点（读 8 个 view + store）这类只读侦察在派发前自己做完、以清单形式喂给子代理，能省它一半探索时间。

## 2026-09-06 AS1 分享链接后端（feat/acp-share-agent 会话反思）

- 哪个坑浪费了最多时间？
  收尾才发现两类与本次改动无关的基线阻塞：验收命令 `cargo test -p acp-*` 在根 workspace 解析不到 apps 独立 workspace（exit 101，需 --manifest-path 或 cd）；apps/cli 存在基线 test 编译红（friend_update 缺字段）与 clippy 1.98 新告警。若开工第一件事就按验收 PATH 空跑一遍验收命令，这些都能在写码前定性。
- skill 有没有提前警告我？
  没有条目覆盖「验收命令基线空跑」与「cargo fmt 在 fmt 门禁不覆盖的独立 workspace 会全 crate 重排存量漂移」——本次被 fmt 波及 10 个 chat/cli 文件，靠 git status 兜住后按纪律拆成独立 style 提交。
- 重来一次会怎么做？
  开工序固定为：worktree → 按协调方 PATH 原样空跑验收命令拿基线 → 读设计 → 动手；每次跑 cargo fmt/clippy/test 前 git status 建立基线快照，收尾 diff 只允许出现自己域内的文件。

## 2026-09-06 反思：endpoint 测试反馈 + toast 可复制可关闭
- 哪个坑浪费最多时间？查证 sonner v2 点击行为绕远路（本地 grep 压缩产物正则不中 → web_fetch 反复超时），最后读 unpkg 未压缩 dist 才定位；应先确认依赖大版本 breaking changes 再设计方案。
- skill 有提前警告吗？没有。bash 默认工作目录语义这次踩实：一次「全绿基线」其实跑在主树上，白信了一轮。已沉淀 techniques.md。
- 重来一次怎么做？① 读依赖 dist 确认行为再设计；② 测试命令一律绝对路径 cd；③ 「点击没反应」类工单先分辨反馈挂在动作上还是挂在全局状态上。


## 2026-09-06 LSG2（IPC 接线+设置页）反思
- 最耗时：环境三连坑——vite-plus node 挂起、worktree 缺 node_modules、vitest 负载假超时；门禁环境先探通再写码更省。
- skill 预警：techniques 已有「bash 全新 shell 必须显式 cd」，但 node 工具链挂起与 hardcoded-copy 行尾注释是新坑，本次已喂回。
- 重来一次：开工先跑通最小门禁（i18n-diff + 单测文件）再动手写码，环境问题前置暴露。

## 2026-09-07 ACP 多工作区与分享打磨轮复盘

- 哪个坑浪费了最多时间？
  run_code 中对大文件的 write 偶发截断（admin_tests.rs、en-US.ts、zh-CN.ts），以及把长模板串和 bash 串混在同一调用导致解析失败；另有 workdir 传错导致 pnpm 在仓库根目录执行。一次 merge 时还把“当前分支 HEAD”误认成 main，导致冲突恢复返工。
- skill 有没有提前警告我？
  已有模板串转义、read 后 edit、workdir 自证和静默空输出经验，但没有明确“每次大文件 write 后必须立刻校验总行数/尾部”，也没有把 merge 前先 pwd + branch + status 作为不可跳过步骤。
- 重来一次我会怎么做？
  大文件优先用小范围 edit 或经过验证的临时脚本，写入后立即 read 尾部和 wc；每个 bash 命令显式在命令内 cd，并先输出 pwd/branch；merge 前先确认目标 worktree 与分支，冲突只在 feature worktree 消化，再做门禁。


## 2026-09-07 Claude 协议翻译评审
- 哪个坑浪费最多时间？
- 设计文档只写翻译 SSE，但未显式说明 serve 的 extract_usage 要求同一事件同时含 prompt_tokens/completion_tokens；逐行核对消费方才发现必须合成 usage chunk。
- skill 有没有提前警告？
- 有先读源码和失败可观测性的原则，但没有专门提示协议翻译必须先逆向消费方解析器。
- 重来一次怎么做？
- 任何协议适配先画输入事件到最终结算字段的消费链，逐字段核对解析假设，再定翻译输出。
## 2026-09-09 GUI 成套设计稿轮（负责人统筹 + 接管出图）

- 哪个坑浪费了最多时间？
  目标误判：把「当前GUI」脑补成 DSH 控制台（我自己的运行环境）而非仓库 apps/gui 的 p2p 桌面应用，5 张废图 + 80 分钟，用户一句「跟 p2p GUI 完全不一样」才纠偏。其次是纠偏可达性：session_link_send 排在 worker 长轮次后不可达，它把接管当成攻击、按撤退协议开了错误目标的新 worktree 继续烧钱，最后靠 interrupt_agent 才止损。两处均已喂回 red-lines/techniques。
- skill 有没有提前警告我？
  没有。references 全是执行层纪律，缺「目标画像必须源码核实/歧义必问」这条元规则与「纠偏可达性」「出图通道选择」条目（本轮已补齐）。
- 重来一次会怎么做？
  设计任务开工三件事前置：① 读仓库定位目标产品画像（menu.def/views/外壳文档）并请用户确认；② 任务书写死画布比例与产品范式；③ 派发后以产物 json 轮询监督、重大纠偏走 interrupt。出图管线沿用最终版：low 探针定尺寸白名单 → 总板先精出 → 页面 generations+内联锚定串行重试 → 逐张读图比对 spec 验收。
