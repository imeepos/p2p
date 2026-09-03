# 固定模板：调试（D）与解题（S）

<!--
  与 techniques.md 的分工：techniques 记散点经验（一条一事），本文件把反复
  出现的骨架抽成机械可执行模板。模板必须是散点条目的收敛形态，且至少锚定
  两个真实实例才够格入库。修订不改写原文：在对应模板下追加「修订 日期」小节
  （只增不改原则的模板版）。
-->

## D1 门禁信号可信度甄别（假绿 / 假红 / 瞬态三分）

适用：任何「命令说绿但行为红」「说红但逐用例全 ok」「偶尔红、复跑就绿」。

固定步骤：
1. 显式收退出码：`cmd > log 2>&1; echo EXIT=$?`——管道尾 `| tail` 的 $? 是 tail 的（红线 21）。
2. 退出码与日志尾部交叉核对：EXIT=0 但 log 含 error/FAILED = 假绿；EXIT 非 0 但逐用例全 ok = 门禁脚本自身嫌疑（make check 的 gate-tests 自测兜这层）。
3. 稳定复现定界：make check 红在哪个目标就单跑哪个目标（cargo test -p X / bash scripts/check/fmt.sh），缩到最小复现。
4. 瞬态定性：失败在隔离复跑中消失 = 构建管线瞬态（pipelined 编译只出 rmeta，doctest 链接要 rlib），记录「复跑通过」后放行，不扩修。
5. 假绿必须找到吞码点修掉（通常是管道/分号链），不许只改这一次的判定结论。

实例锚点：2026-09-04 `cargo test | tail` 掩盖编译错误报 exit 0（bash-41）；2026-09-04 主树 llm-share-offer doctest E0463 复跑即绿。

## D2 主树门禁红：所有权三步取证

适用：并行多会话环境，主树 make check 红，先判定是不是自己引入的，再决定动手还是报告。

固定步骤：
1. 范围取证：`git diff <我开工时main的tip>..HEAD --stat -- <失败路径>` 为空 = 非我引入。
2. 归属定位：`git log -1 --format="%h %ad %s" -- <失败文件>` 拿最近触碰提交，读 message 判断意图。
3. 处置分流：
   - 机械零行为（fmt / import 排序）→ 代修，提交信息注明代修缘由与属主。
   - 语义缺陷 → 不抢修（crates 归属边界），报告里给属主留 file:line 与复现命令。
   - 瞬态 → 按 D1 第 4 步定性后复跑放行。

实例锚点：2026-09-04 make check 连红两次（fmt-check 与 doctest），取证均为并行会话产物；fmt 代修 fd15150。

## D3 编译破损时间线对撞（谁改坏了谁）

适用：某符号/签名/字段突然编译不过，定义方与消费方分处两文件。

固定步骤：
1. 定义点与消费点分别 `git log -1 -- <file>` 比最后改动，时间晚者即引入方。
2. 盲区判定：引入方提交为何没被门禁拦——查该 crate Cargo.toml 有无空 `[workspace]` 表（脱离根 workspace 则根 fmt/clippy/test/panic-hygiene 全不覆盖）+ fmt.sh 的 cd 基准。
3. 修法：适配消费点（冻结形状只走加法），失败路径补 warn 日志与可读 Err，不许静默。
4. 给盲区补门禁的提案随报告提出（如独立 crate 的 clippy+test 挂进 gui.sh）。

实例锚点：2026-09-04 EchoHandler E0423——6c4d882 改 p2p-cli 形状，src-tauri 消费点一天无人编译才暴露，补 fix(gui-tauri)。

## D4 「GUI 里为什么有 X」双路并查

适用：邻居表幽灵条目、离线节点、多出来/少掉的数据类问题。

固定步骤：
1. 不先读代码：一路查活进程拓扑（lsof 端口监听 + ps 进程树起点），一路查数据来源（出厂默认配置 / 发现协议语义 / mock 残留）。
2. 来源定准后先判语义：是不是设计如此（本项目「离线」= 未连接且 10 分钟无正向证据；地址簿只增不减无删除路径）。
3. 语义排除后才轮到读代码找 bug。

实例锚点：2026-09-03 邻居表 127.0.0.1 归属判定；2026-09-04 离线节点双路并查（techniques 同名条目）。

## D5 GUI 渲染/状态异常执行序

适用：白屏、崩溃、无限重渲、toast 不弹、显示与状态不符。

固定执行序（从快到慢）：
1. vitest 输出搜「渲染异常」（ErrorBoundary componentDidCatch 的 console.error 带根因消息）。
2. Maximum update depth 且栈里 forceStoreRerender/updateStoreInstance = store 快照引用漂移——派生列表按源引用 memo 化。
3. 「状态对、显示错」先写 probe 测试锁 DOM value 再动手分类（受控 vs register）。
4. toast 类：sonner 在 jsdom 异步 mount，用 await screen.findBy*；测试间 afterEach 补 toast.dismiss() 清模块级队列。
5. 整树白屏复现：vi.stubEnv("VITE_MOCK_IPC","1") 后 await import("../main")，断言 host 有 main 且无 ErrorBoundary 兜底文案。

实例锚点：techniques 2026-09-02/03 五条散点（selectPeerList 白屏、relay 页白屏、settings-defaults probe、sonner 两条）。

## S1 GUI 新特性五步走（展示层属性标准流程）

适用：给 p2p-console 增加本机节点属性/视图能力。

固定步骤：
1. 架构定位：纯展示属性 → GUI 桥接层（apps/gui/src-tauri + src），不进底座 crates；冻结契约只加法（新增字段/新增命令），先写契约加法小节。
2. worktree + 取证：fetch 后 grep -rn 新概念名于 apps crates docs，确认无同域工作再开工。
3. 注册先行：i18n 键位（zh-CN/en-US 同一提交）独立小提交，不埋进 feature 提交。
4. 分层落地（每层同签名同规则）：契约 docs → 后端（持久化镜像 ConfigStore 形态 + 校验 + 单测）→ 前端（ipc-types → ipc → mock-ipc → store → 组件 → 页面挂载）。
5. 收尾：scope 双门禁（src-tauri cargo clippy+test；bash scripts/check/gui.sh）→ rebase main → 零重叠检查（diff 旧tip..新HEAD 不含自己文件）→ 主树 ff-only → 四步清理。

实例锚点：2026-09-04 节点资料 NodeProfile 全流程（7 提交，i18n 两笔先行、契约一笔、后端两笔、前端一笔）。

## S2 复用锚点先行（不造轮子的机械化）

适用：写任何新组件/store/持久化/测试前。

固定步骤：
1. 动手前先答：仓库里最接近的既有形态是什么？写出锚点文件路径。
2. 锚点速查（2026-09-04 时点）：持久化读写 → config.rs 的 ConfigStore（原子写/损坏回退/告警）；独立卡片局部保存 → appearance-card + profile-card 的 draft 模式；store 测试 → update-store.test（vi.hoisted + setState 重置）；组件测试 → 真 i18n（import "@/i18n"）+ vi.mock("@/lib/ipc")；命令薄封装 → commands.rs 既有命令；mock 镜像 → mock-ipc 既有方法。
3. 复制形态、替换内容，不引入第二套抽象。

实例锚点：NodeProfile 的 ProfileStore 逐方法对照 ConfigStore；profile-card.test 对照 update-store.test 模式。

## S3 提交拆分五问（可 revert 纪律机械化）

适用：任何一次 commit 前自检。

固定步骤：
1. 这个提交能被单独 revert 而不伤邻居吗？不能 → 拆。
2. 含注册类文件（menu.def.ts / App.tsx / i18n types+locale / fields.md）吗？有 → 注册改动独立小提交。
3. fix 和 feat 混在同文件吗？是 → 临时摘除法拆（摘 feat hunks 验证编译 → 提 fix → 恢复 → 提 feat）。
4. 提交信息是 type(scope): subject 加机理正文吗？正文只写 why。
5. 提交后 git log --oneline -1 核对落点与分支（红线：skill 沉淀禁直落 main）。

实例锚点：2026-09-04 state.rs 同含 fix+feat 拆两提交流；i18n 两次独立注册提交。
