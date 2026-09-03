# Red Lines

<!-- 格式：禁止 X，因为 Y 发生过。真的付出过代价才记。 -->

- 禁止在已存在 .env 的目录里 `git add .` / `git add -A`：本工作区 .env 存放密钥，
  首次提交必须先写 .gitignore（.env）并使用显式路径 add（2026-09-02 p2p 仓库初始化时规避）。
- 禁止用手工枚举关键词（PASS/SECRET/TOKEN）的 sed 展示 .env：本机 .env 含 *_API_KEY 明文（OPENAI/AMAP/E2B），枚举漏掉 KEY 类导致密钥进对话记录（2026-09-02 实证）。规则：默认把每行 KEY=VALUE 的 VALUE 全部打码，只显式放行非敏感键（HOST/USER/REGION/DOMAIN/URL）。
- 禁止反向同步用 git merge main 留 merge bubble：派单工作流的规则 5 是 rebase main，merge 提交会污染 main 历史还要多占一个提交号（2026-09-02 D-E4 2443366 实例，协调会话裁定不 revert、下不为例）。
- 禁止跨 crate 改动不先报协调会话：派单声明的 crate 范围是边界，噪声源在别的 crate 接线层也不能直接动手——先回报再改（2026-09-02 D-E4 6a4df8c 触及 crates/p2p facade 被点名，本次因无并发冲突豁免）。
- 禁止依赖 .env 的部署脚本只在主树验证：.env 被 gitignore，worktree 里没有副本，脚本在 worktree 内运行必死于凭据加载——这类脚本必须有显式外部指定入口（如 DEPLOY_ENV_FILE=<主树>/.env）并在头部写明（2026-09-02 ECS 部署实录，静态自检 grep .env 当场拦下）。
- 禁止 `git commit -m $(cat <<'EOF' ... )`：命令替换被词切分，commit 只收到第一个词却静默不报错（HEAD 不动、文件留在暂存区），本次靠提交后核对才发现空跑。规则：多行 message 一律 `git commit -F - <<'MSG' ... MSG`，提交后必须核对 git log（2026-09-02 E4 K 会话实录）。
- 禁止在 monorepo 子包之外的目录跑 pnpm（G-B2 当轮连犯两次，NO_IMPORTER_MANIFEST
  让验证假红、提交未验证落盘）：workdir 必须显式指到子包，构建验证与 git 提交
  永远拆成两条独立命令，提交前必须先看到本轮 build 的真实退出码 0（2026-09-02）。
- 禁止把「跑验证 + git add + commit」串进同一条分号链命令：链上任何失败都会被
  吞掉继续提交，revert 纪律的前提是提交前验证（2026-09-02 两次实例后固化）。
- 2026-09-02 G-A3：skill 经验沉淀（.agents/skills/ 下文件）的提交必须放在**自己的 feature 分支**随任务合并，禁止直接提交到主树/main——两次（e8cb4cb/48d170b）直接落 main 挡了协调者的 ff 线性合并，被迫手工对齐。任何"写完立刻 commit"都要先核对当前分支与提交落点是否侵犯共享主线。
- 禁止协调者合并命令链用分号续接 branch -d / push --delete：ff 失败后链条继续会把未合并分支连删两次（2026-09-03 p2p GUI 一晚两次，靠回报中的 tip 哈希才救回）。合并尝试必须用 && 短路，或把验收/合并/清理拆成独立调用；删分支前先数 main..<branch> 确认为 0。
- 禁止只跑 cargo 门禁就宣称 GUI 任务完成：GUI 变更必须 gui-check（lint+build+vitest 含启动冒烟）绿了才算验收，make check 已含 gui-check（2026-09-03 白屏事故固化）。
- 禁止 zustand selector 把每次新建的数组/对象（Object.values/sort/map/filter 的直接产物）当 useSyncExternalStore 快照返回：引用漂移让 React 无限重渲直至整树崩溃。派生列表按源引用 memo 化，或消费侧 useShallow 包裹（2026-09-03 selectPeerList 白屏根因）。
- 验证命令禁止让管道末端吃掉真实退出码：`cmd | tail` 的 $? 是 tail 的；必须 set -o pipefail 或无管道直接看退出码（2026-09-03 复盘时管道掩蔽测试失败差点假绿）。
- 禁止把「前端错误只进 console」当作可观测：Agent/外部进程读不到 console，用户看到的报错
  就是系统盲区（2026-09-03 用户点名系统缺陷）。前端错误必须落盘（Tauri 文件/localStorage）
  或经桥接命令可读，全局 error/unhandledrejection/console.error/ErrorBoundary 四入口都要接。
- run_code 模板字符串里写含反引号的 markdown（README 代码块、`inline code`）必炸
  "Expected ',', got 'ident'"：内容改单引号字符串数组 join(反斜杠+n) 再拼（2026-09-03 README 实录）。


- 2026-09-02（协调者裁定）：skill 沉淀提交走 feature 分支再合并，禁止直落 main（W6 的 docs(skill) 直落 main 属存量先例，不构成豁免；S3 的 23abb8b 已被保留但记档下不为例）。
- 2026-09-02（协调者裁定）：gitignore 内的验证产物（截图/报告 JSON 等）若需留档，必须在关单/worktree 清理前显式迁移到入库路径或另行声明放弃——S3 的 28 张矩阵截图随 worktree remove 丢失（原始产物有损，文档本体无损）。
- 禁止启动/路由冒烟只踩默认路由：每个注册路由都要真实导航并断言不落入 ErrorBoundary 兜底，且断言含「数据就绪才出现」的标记而非骨架屏——relay 页白屏（FormProvider 外解构 useFormContext 的 control）越过只测 dashboard 的启动冒烟直达用户（2026-09-03 实证）。
- 禁止连接生命周期事件（PeerConnected/PeerDisconnected）的发起点与连接真实状态脱钩：断开事件只能由「该连接确已出池」触发，被顶替/被拒收的连接、发现层缓存过期都不得谎报；配套的挂断/关停/关停全节点路径必须主动补发断开，否则要么闪断要么状态卡死（2026-09-03 拨通闪断实证）。
