# Orchestrator 账本 — tunnel 泛化波（2026-09-12）

计划：`.orchestrator/2026-09-12-tunnel-any/proposal.md`（含契约裁定）
协调会话：session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8
基线：main @ a9ea63e1（== origin/main，开工时核对）

## 任务

- TA 契约+签名桩（architecture, 60）：dispatched（sessionId=session-0272ac3d-7a10-4f61-a371-4dacb2e007b0，
  分支 feat/wta-tunnel-contract / worktree .worktrees/wta-contract / 基线 a9ea63e1 / 任务书 brief-ta.md）
- TB 下沉实装+GUI 切换（code-quality, 120）：dispatched（sessionId=session-1a92e65c-3bc9-4f58-a7ff-530b1f9e5687，
  分支 feat/wtb-proxy-sink / worktree .worktrees/wtb-sink / 基线 ba45afd8 / 任务书 brief-tb.md）
- TC CLI connect+itest（code-quality, 120）：pending（**实依赖 TB**——itest 需
  local_proxy 真实现，桩态必红；TB 合并后与 TD 并行派）
- TD GUI 通用入口（frontend, 120）：pending（等 TB，与 TC 并行）
- TE e2e 证据（code-quality, 100）：pending（等 TC+TD）

## Facts

- F1 协议 tunnel/1 已全通用（target/allowlist/字节泵/WS 透传），registry 零改动。
- F2 被访侧就绪：p2pctl tunnel serve 多目标 + GUI serve 单目标。
- F3 访侧反代核心 ~1400 行在 apps/gui/src-tauri/src/tunnel/，核心已参数化，
  DSH 耦合仅在 url.rs（token/启动 URL）与命令命名层。
- F4 CLI 无访侧命令。
- F5 规范页 §8 写 LocalProxy|crates/p2p-tunnel，实现漂移在 GUI，§8 状态段滞后。
- F6 GUI tunnel_open_dsh = DSH 启动 URL 专用；remote-access 页 DSH 专用。

## Guesses（带过期条件）

- G1 桩签名可从 GUI 现码机械提炼且不漏泛化点 — 证伪条件：提炼中发现
  LocalProxy 依赖 tauri/前端类型（BLOCKED 回报）。
- G2 TB 迁移零行为变化（测试平移即绿）— 证伪条件：迁移后任一既有测试红
  （此时差异即缺陷，回炉）。
- G3 TC itest 复用 tunnel_common 裸流工厂即可跑 HTTP+WS 双服务 — 证伪条件：
  WS echo 用例在 yamux 线上出现半关/缓冲死锁（KI:609 同族，届时按真实 TCP 探针补）。

## Rulings

- 见 proposal.md「契约裁定」四条（local_proxy.rs 命名 / token 空串语义 /
  DSH 语义留 GUI / connect 暂不进 TSV）。

## 进度账本（每事件一行，五问裁决）

- 2026-09-12 06:55 事件：波次开工（五问：完成【否，计划落盘】；空转【否】；
  进展【事实核查 6 条+坑点调阅+SESSIONS.md 认领】；下一步【派 TA】；指令【见
  brief-ta.md】）
- 2026-09-12 06:57 事件：TA 派发完成，基线核对 main==origin/main @ a9ea63e1，
  worktree 仅主树无残留（五问：完成【是，已投递】；空转【否】；进展【契约裁定
  四条冻结进 proposal】；下一步【等 TA 回报，回报后先验落盘迹象再复核签名表；
  TB/TC 任务书等 TA 签名表填充，不预写占位符】；指令【—】）
- 2026-09-12 16:07 事件：TA checkpoint@30 收到 → 验活通过（五问：完成【否，中期；
  桩已提交 b2952fbc，文档面改而未提交】；空转【否，workspace check 在跑】；进展
  【ConnAudit→PumpAudit trait 缝是新设计点，终审时重点复核；编号落点 §19.3 约束 9
  与任务书 §19.9 有偏差，已要求回报映射关系】；下一步【等终报，届时逐字复核签名表
  +diff 复核 trait 缝设计】；指令【已 send：签名表逐字/登记独立提交/编号映射注记】）
- 2026-09-12 16:15 事件：TA 终报 DONE → 独立复核通过 → 合并 main @ 5770c15a
  （五问：完成【是】；空转【否】；进展【5 提交链桩/文档/PROGRESS/喂回分离，
  我侧重跑三门禁 EXIT=0，diff 逐项合格：PumpAudit 缝采纳、tunnel_open 契约
  含「禁空串拼装 ?token=」补强、specs §8 漂移登记规范】；下一步【派 TB】；
  指令【—】）
  - 过程注记①：TA 在 DONE 回报后追推 2d0b4b8d（按我 checkpoint 要求升级
    PROGRESS 为逐字源码形态）未再回报——内容合规、流程记一笔（终报后不再
    静默追加推送）。已吸收进 main @ ba45afd8（ta-PROGRESS.md 新路径，
    diff 逐字节一致），分支安全删除。
  - 过程注记②：根路径 PROGRESS.md 会随多卡合并互相覆盖 → 1ef3a4d8 迁入
    波次目录；后续各卡任务书统一指定 .orchestrator/2026-09-12-tunnel-any/
    <卡>-PROGRESS.md。
- 2026-09-12 16:18 Ruling：波次依赖重排 TA→TB→(TC∥TD)→TE — 依据：TC 的
  itest 消费 local_proxy 真实现而非仅签名，桩态必红，TB 是 TC 实依赖 —
  代价：TC 推迟一拍，并行度由 TB 期间预写 TC/TD 任务书补回。
- 2026-09-12 16:18 Ruling：connect 首版仅 bind(127.0.0.1:0) 随机端口 — 依据：
  LocalProxy bind 形状是 TA 冻结契约（§19.3-1/§3.3），确定性 --listen 属
  契约加法候选，本波不做 — 代价：运维脚本须从 ready JSON 行读端口（可接受）。
- 2026-09-12 16:20 事件：TA 在途回应（三点提醒处置说明）与合并收尾交叠
  （五问：完成【是，全部已处置】；空转【否】；进展【编号映射对账接受；追补
  提交已吸收；已 send 告知终态并令其静默】；下一步【等 TB/全量门禁】；指令【已发】）
  - TA 验收裁定：PASS。归档押后：等主树全量 make check 绿（TA 合并兜底）再归档。
- 2026-09-12 16:45 事件：TB checkpoint 40/120（五问：完成【否，代码落地门禁
  在跑】；空转【否】；进展【填埋+13 条测试迁移+GUI 切换全绿，三分歧点提出：
  ended_at==0 哨兵/SessionLog 换型/getrandom 入 crate】；下一步【终报核三证据：
  哨兵无消费方 grep+映射测试名、pub 面对照、lock 同提交】；指令【已 send：
  三决策准许+证据要求+Cargo.lock 属预期 diff 项】）
  - 预裁定（终报复核口径）：三分歧点方向均接受，验收只认证据；新文件行数
    ≤300 与 diff 范围（含 Cargo.toml/lock）照任务书口径核。
- 2026-09-12 17:00 事件：TB 终报 DONE_WITH_CONCERNS → 我侧复核主体通过 →
  合并 main @ d9f366ca（五问：完成【是】；空转【否】；进展【diff 面与自报
  逐项一致、禁区零命中、30 passed 我侧重跑一致、哨兵映射唯一消费方=GUI From
  已测、两项 CONCERNS 均有证据准许】；下一步【合并后全量门禁】；指令【—】）
- 2026-09-12 17:00 事件：★主树全量门禁红（panic-hygiene）→ fix-first
  - 证据：gate-tests/panic-hygiene FAIL，crates/p2p-tunnel/src/local_proxy/
    mod.rs:100 expect()（非测试路径禁 unwrap/expect/panic）。
  - 根因定责：**协调者任务书缺陷（重犯）**——验收清单未含 make 级门禁全量
    口径（上一波 W-T2 clippy/cli-parity 同款教训「任务书必须含全量门禁清单
    或明确豁免」我没落进 TB 任务书）；代码侧成因=GUI 原 proxy.rs 同款 expect
    一直在 panic-hygiene 扫描面外（src-tauri 双 workspace），下沉进 crates/
    即撞线。
  - 处置：打回 TB 修复轮 1（修法=bind 时消 err 存 SocketAddr 字段，消除
    panic 路径）；TC/TD 派发押后至主干复绿（合并队列 TB-fix 优先）；bash-133
    （ba45afd8 旧基线全量）已 kill——TA 面是 docs+桩（无 expect），该红与 TA
    无关。
  - TB 提的三条 skill 喂回（穷尽 match enum 红线/双 workspace 门禁路径/
    lock 豁免条款）+ 本条任务书缺陷，记入波末统一喂回清单。
- 2026-09-12 17:02 事件：TB 证据附页（写于打回令前，在途交叠）——三项证据
  与我侧独立复核一致，采纳入档；已 send 重申修复令有效，等其 panic-hygiene
  修复回报。
- 2026-09-12 17:16 事件：★TB 修复轮 1 通过并合并，主干复绿在途
  - 修复分支 feat/wtb-proxy-sink-fix @ 96514234（+7/-2 单文件）：bind 时就地
    消 io::Result 存 addr 字段，expect 消除，零行为变化。
  - 我侧独立复核：diff 逐行核过；panic-hygiene 自测 11/11 + 真门禁 PASS、
    cargo test 30 passed、fmt——全绿后合并。main @ 96514234 已推。
  - TB 验收终裁：PASS（含修复轮 1）。其 skill 喂回建议追加一条：GUI→crate
    平移前先对目标 crate 跑全量 make check 口径（panic-hygiene/clippy/fmt）。
  - 过程注记：worktree remove 前台 60s 超时（上波已知坑），已按先例转后台
    rm -rf + prune + 删分支（bash-149）。
  - 合并后全量 make check 后台运行中（bash-150，/tmp/tunnel_any_main_check_tbfix.log）。
- 2026-09-12 17:17 事件：TC ∥ TD 并行派发（五问：完成【是，双 greeting 投递】；
  空转【否】；进展【主干绿在途认证、两任务书已补 make 级门禁条款】；下一步
  【等双卡检查点与全量门禁结果】；指令【任务书即指令】）
  - TC（session-dc256684-9580-4bb7-9caf-b401293510fb）：分支 feat/wtc-cli-connect，
    文件域 apps/cli + crates/p2p-itest + ai-guide。
  - TD（session-afaa0b9d-2b79-4248-a128-7e56ffbc62a8）：分支 feat/wtd-gui-open，
    文件域 apps/gui + cli-parity.tsv + i18n。
  - 冲突预判：两卡文件域零叠；cli-parity.tsv 只有 TD 动（TC 明令零改动）。
- 2026-09-12 17:18 事件：TB 现场清零确认（worktree/本地远端分支全删，
  NO_WTB_LEFTOVER）；TE 任务书预写落盘（brief-te.md：非 DSH 真实 HTTP+WS
  服务、CLI+GUI 双访侧面、九项证据链、102 现场清理清单）——TC+TD 合并后
  即派。当前在途：TC、TD、bash-150 全量门禁。
- 2026-09-12 18:09 事件：★TC 终报 DONE → 复核通过 → 合并 main @ 80a63cdd
  （五问：完成【是】；空转【否】；进展【6 提交链分离合格；style 021ab236 抽验
  纯格式化且 7 文件在 apps/cli/** 域内——apps/ 在 fmt 门禁外属实（上波
  acp-common 先例），存量漂移真实、独立可 revert 处置正确；itest 4 passed +
  panic-hygiene 我侧重跑一致；根 Cargo.lock 零改动自裁定采纳】；下一步
  【等 TD；全量门禁重起】；指令【已 send 验收回执+uuid/getrandom Minor 注记】）
  - Ruling：TC 的 SHA-1/base64 测试内自足实现（替代 dev-dep 保 lock 纪律，
    RFC 向量锚定）采纳为仓内先例，进波末喂回候选。
  - Minor 注记：connect nonce=uuid v4 vs GUI=getrandom，访侧随机源不一致，
    后续统一候选（不返工）。
  - itest「镜像装配」duplication（apps/cli 无 lib 目标）已由 tc-PROGRESS 登记
    同步义务——抽 p2p-cli 复用模块为后续候选，backlog。
  - 全量 make check 重起后台（bash-178）；TC 现场后台清理（bash-179）。
- 2026-09-12 20:02 事件：★main @ 80a63cdd 全量门禁 EXIT=0（5696 行十二道
  门禁完整；3 处 FAIL/Error 命中核为门禁自测夹具与错误路径测试预期 stderr）
  ——TA+TB+TC 合并认证闭合。三会话归档完成（session-0272ac3d / session-1a92e65c
  / session-dc256684，3/3 成功），worktree/分支此前已清零。归档队列为空。
  在途仅 TD（session-afaa0b9d）；TE 任务书备料完毕，TD 合并后即派。
- 2026-09-12 20:36 事件：TD 终报 DONE_WITH_CONCERNS → 内容面复核通过、
  流程一项打回（五问：完成【是，内容合格】；空转【否】；进展【merge bubble
  183c4cab 违反 2026-09-02「反向同步一律 rebase」裁定，打回线性化】；下一步
  【等 rebase 后新 tip，复核即合并】；指令【已 send：rebase + 丢弃冗余
  7c8cb2c5 + 五门禁复跑 + force-with-lease】）
  - 内容复核记录：lib.rs 插入序合规（KI:601 规避正确）；TSV tunnel_open
    exempt 行理由成立（引 §19.3-8/9，serve 先例同构）；范围 16 文件零越界；
    ai-guide 净零（merge 冲突取 TC 版正确）；concern-1 截图缺口转 TE 承接；
    concern-3 vitest forks worker 坑入喂回清单。
- 2026-09-12 20:59 事件：TD 上报规范文本矛盾（AGENTS.md 旧文本 merge main vs
  2026-09-02 裁定 rebase）→ 采纳修正：docs/agents-reverse-sync @ 418a02d9
  已合并 main 并推（两处统一 rebase 口径，裁定出处入条文），worktree/分支
  清理完毕。TD 根因定性=文档矛盾非执行过错，打回维持（rebase 口径为准）。
  - 副作用与处置：main 前进后 TD 分支（c0d2b178 基于 80a63cdd）需再同步；
    其 16 文件不含 AGENTS.md，rebase 必无冲突——按 W-T5 先例由协调者对冻结
    分支代 rebase（等 bash-186 cargo 面绿后一并执行：rebase→push→合并）。
  - 前台聚焦证据：generic-tunnel-card 三态测试 3/3 绿（我侧 vitest 实跑）。
- 2026-09-12 21:16 事件：★TD 修复轮 1 通过并合并，main @ fb70de4f
  （五问：完成【是】；空转【否】；进展【bash-186 cargo 面我侧实跑绿
  （141 passed + clippy -D warnings 4m27s）→ 协调者代 rebase（W-T5 先例，
  5 提交无冲突重放 fb70de4f）→ force-with-lease 推 → ff-only 合并推 main】；
  下一步【全量门禁 + 派 TE】；指令【已 send 验收回执，TD 静默】）
  - TD 终裁 PASS。其 16 文件不含 AGENTS.md，rebase 零冲突符合预判。
  - 全量 make check 后台（bash-192，/tmp/tunnel_any_main_check_td.log）；
    TD 现场清理后台（bash-193）。
- 2026-09-12 21:16 事件：TE 派发（session-0fafe7f2-774f-42b0-8629-1f895c0d1c32，
  分支 feat/wte-e2e-evidence / 基线 fb70de4f / 任务书 brief-te.md）——最后
  一卡：102 无头非 DSH 真实 HTTP+WS 服务，CLI+GUI 双访侧面，九项证据链。
  波次仅余 TE 在途。
- 2026-09-12 21:48 事件：★TE 终报 DONE → 复核通过 → 合并 main @ 8f8e0daf
  （五问：完成【是，九项全 PASS】；空转【否，89/100 预算含三次检查点】；进展
  【抽查：17 文件零越界、EVIDENCE.md 命令原文+退出码齐、raw 12 份截图 2 张
  在位——全波最高质量证据】；下一步【最终全量认证+波次收口】；指令【已 send
  验收回执，观察项 ①②③记 backlog、④并入上波 remote-access 控制通道注册卡】）
  - TE 终裁 PASS。现场清理后台（bash-204）已完成 NO_WTE_LEFTOVER。
  - 负载假红记录：bash-192 全量红（p2p-chat invite_flow is_none 时序断言）
    三角定性负载假红（同代码态 80a63cdd 全绿+隔离 6/6+TE 并行重载），最终
    tip 重跑认证中（bash-203）。
- 2026-09-12 21:48 事件：波末统一 skill 喂回合并 main @ 36183665（查重后纯
  追加 15 行：known-issues 扫描面平移坑+Rust 时序假红三角定性、red-lines
  任务书全量门禁清单红线、techniques 跨机 e2e 证据链标准路径、lessons 冻结
  分支协调者代 rebase 先例）。main 仅前进 docs-only（.agents 不在门禁扫描
  面），8f8e0daf 的全量认证对波次代码面仍有效。
- 2026-09-12 22:18 事件：TE 追加收口（⑤终态对账口径确认+⑦GUI 双负例截图）
  → 复核合并 main @ 8d64663f（五问：完成【是】；空转【否】；进展【单提交
  7 文件零越界、badpeer 错误态+502 负例双截图、双侧 rejected 审计逐字对账、
  二轮双端清零】；下一步【等 bash-203 全量认证，绿则归档 TE 收官】；指令
  【已 send：③open 即报错属契约加法候选记 backlog 不立项、超支 3 准销】）
  - Ruling：「open 时预检被访侧 allowlist」为契约加法候选（现行为 CLI↔GUI
    同构已文档化，本波不立项）。TE 技术预判随卡记档：预检=GUI open 路径新增
    一次被访侧往返（现 LocalProxy 即刻起动与 §19「进程活=受理开启」一致），
    若立项按契约加法走红绿，TE 可出跨机实证。
  - TD 转来的截图缺口就此彻底闭合。TE 终裁 PASS（含追加轮）。
- 2026-09-12 21:24 事件：TE 检查点 1/3（五问：完成【否，准备就绪】；空转【否】；
  进展【102 裸拷贝无 git 自裁定改走 bundle 重传；GUI 注入链按 TD 新 DOM 适配；
  102 双服务 18081/18082 拉起中】；下一步【等 60 检查点】；指令【已 send：
  清单补 /tmp/te-p2p 与 npm 目录，禁持久化配置】）
- 2026-09-12 21:34 事件：TE 检查点 2/3（五问：完成【否，②④⑥①四项 PASS，
  ③GUI 注入驱动进行中】；空转【否】；进展【CLI HTTP cmp 逐字一致/WS 文本+
  二进制双向对证/双侧回环证明含 mac 负探针 exit=7/mDNS 跨机发现】；下一步
  【等终报：GUI openUrl 截图+审计对账+负例+优雅收口+EVIDENCE.md】；指令
  【已 send：⑤对账取终态（哨兵口径）、⑦负例 CLI+GUI 各采一例补截图缺口】）
- 2026-09-12 21:41 事件：bash-192 全量门禁红 → 定性负载型假红，隔离复绿
  - 红点：p2p-chat --test invite_flow::invite_reject_notifies_inviter_and_keeps_none
    （assert is_none 时序断言，10.7s 处 FAILED）。
  - 三角证据：①fb70de4f 与全绿的 80a63cdd 代码差仅 TD 的 16 文件（apps/gui/
    tsv/docs）+ AGENTS.md，p2p-chat 路径零改动；②隔离复跑 invite_flow 6/6 绿
    （我侧实跑）；③红时 TE 正在本机并行 release 构建 + GUI 实例（负载上下文，
    W-T4 rendezvous flake 与 W-T2 期负载假红同族）。
  - 处置：不回滚不重跑即刻全量；TE 收官合并后对波次最终 tip 重跑全量 make
    check 作波次认证。
- 2026-09-12 22:49 事件：bash-203 波次终认证绿（五问：完成【是】；空转【否】；
  进展【全量 make check 于波次 tip 8d64663f 一次通过 MAIN_MAKE_CHECK_EXIT=0；
  尾段实测 gui-tauri-check PASS / panic-hygiene 221 文件零违例 / protocol-
  registry 550 文件 17 id 全核对 / cli-parity 83 命令映射 69 豁免 14 /
  ai-docs-sync 101 叶子 347 参数全对；21:41 负载假红定性就此终验】；下一步
  【归档 TE/TD、账本闭环、释放 scope】；指令【无】）
- 2026-09-12 22:55 终局裁定：tunnel 泛化波 PASS，六卡全部验收合并
  - main = 8d64663f == origin/main；波次代码认证基 8f8e0daf（其上仅
    .orchestrator docs-only 追加，不在门禁扫描面，认证延续有效）。
  - 六卡：TA 本地代理核心下沉 crates/p2p-tunnel/src/local_proxy/（head 重写/
    pump 审计/session 审计 ended_at==0 哨兵）；TB GUI tunnel_open（token=""
    零组装，DSH 语义永留 GUI 层）；TC p2pctl tunnel connect（随机端口、
    SIGTERM 优雅、ready/stopped JSON 行）；TD GUI generic-tunnel-card +
    cli-parity 豁免行登记；TE 102 跨机 e2e 全证（HTTP cmp 逐字/WS 文本+二进制
    /双侧审计对账/双负例截图）；docs AGENTS.md 反向同步条文改写（禁 merge）。
  - 存档补齐：波内完成的 dev-orchestrator skill 升级（找活扫描九源+等人三分
    +中途修正协议）随终局一并入库；TE(0fafe7f2)/TD(afaa0b9d) 会话归档，
    tunnel 域 scope 释放。
  - backlog（本波不立项，已逐条记档）：GUI serve 多目标；多会话并发
    tunnel_open；确定性 --listen；open 时被访侧 allowlist 预检（契约加法，
    红绿+TE 可出跨机实证）；remote-access 控制通道 ROUTES 注册；/screenshot
    遮挡陈帧；rejected 审计双印；nonce 来源统一（uuid vs getrandom）；itest
    镜像装配去重进 p2p-cli lib；前波 9 会话待用户归档。
