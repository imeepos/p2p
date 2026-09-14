# 波次账本：2026-09-13 权限统一收口 + 服务总控

> 协调者: session-cd1cdce3-339d-4eb5-9414-d34cf87e51a1 | 分支前缀: feat/wsm-*
> 计划版本: v0（骨架，待用户裁决 scope 后升 v1 锁定）

## Facts（已核实）

- authz 底座已收官：9 权限闭集 + 四内建角色 + 自定义角色 CRUD（GUI §18 已上线，
  main @ 0d257fe3 之后）。PEP 已接：llm-share 闸1 / a2a 双查 / acp 双查 / repair mint 前置。
- chat.send / chat.attachment 已登记未接判定（好友即通，设计 §14 开放问题 4）。
- 服务开关现状（初勘，待盘点卡核实）：
  - mdns 发现：GuiConfig.enable_mdns，持久化，GUI settings Switch（network-card.tsx）。
  - tunnel 被访：TunnelServeGate.set_enabled，会话态默认关，GUI remote-access 页（契约 §762 行）。
  - a2a 发布：per-agent visibility，publish 硬编码 enabled:true（apps/cli/src/a2a/publish.rs:65）。
  - llm-share 借出：闸1 已切 authz(llm.borrow)；serve 侧 gate 待核（llm_share/serve/gate.rs）。
  - relay / rendezvous / observation：地址列表隐式开关（有地址即启用），无显式总闸。
- 项目无 TODO.md（本轮已建）；无 docs/notes/adopted/，不可逆决策记录在 .orchestrator/<波次>/。

## 决策请求（已发用户，等裁决）

- D1 服务总控形态：A 统一注册表(荐) / B 散点补开关 / C 注册表+运行时热切换。
- D2 权限收口范围：A 行为兼容收口含 chat.send 真接(荐) / B 全收口含 tunnel.visit / C 只盘点不改判定。
- D3 开关持久化与默认：A 持久化+安全服务出厂默认关(荐) / B 全会话态默认关 / C 全持久默认开。

## 裁决记录（用户 2026-09-13 单选确认，全按推荐项）

- Ruling: D1=统一服务注册表（A 方案）；D2=行为兼容收口（chat.send 真接，tunnel.visit 不做）；D3=持久化+安全默认关。
  — 用户 ask_user_question 三题单选直接确认 — 错误代价：散点开关重复造轮子 / 权限统一落空 / 安全默认缺失。
- Ruling: 计划 v1 锁定（两轮：A 契约单飞 → B1-B5 并行，首批服务闭集 10 项）。
  — inventory.md 事实 + 上述 D1-D3 推导 — 错误代价：契约未钉先并行 = 集成点失控。

## 事件日志

| 时间 | 事件 | 裁决（五问） |
| --- | --- | --- |
| 13:02 | 波次开工：事实源对齐完成（authz 设计 §1-14、GUI/CLI/装配面初勘） | 完成对齐；下一步：盘点卡派发 + 用户裁决 D1-D3 |
| 13:05 | 盘点卡派发（subagent 3be2b01a，只读 architecture） | 等待窗口用于用户裁决提问 |
| 13:20 | 用户三裁决落定（D1/D2/D3 全按推荐） | 计划期可锁定，转 plan.md v1 |
| 13:50 | 盘点卡 DONE 验收通过：inventory.md 81 行三表+13 开放问题，抽查与我初勘交叉一致（enable_mdns/tunnel gate/lanOnly 断链/a2a stub） | 事实基础成立；六个收口级新事实并入计划：lanOnly GUI 断链（问题1）、tunnel serve GUI 零调用（问题2）、a2a stub 双轨（问题3→候选池）、GUI auto-bind 缺失（问题4→B5）、llm-share 无关闸（问题5→B2）、内建 rendezvous 服务端未暴露（问题7→首批清单） |
| 13:53 | plan.md v1 落盘（锁定区 A+B1-B5、待细化区、明确不做） | 下一步：派 A 卡（session_link_create） |
| 13:55 | 派发前机械核查通过：gui-contract §20 空闲（末章 §19:746）、crates/p2p-service 不存在 | 契约号与 crate 名可用，无撞号 |
| 13:55 | A 卡派发 session-c97ab513-4aba-4771-843c-3648f5d4f8f9（分支 feat/wsm-a，worktree wsm-a，8 检查点预算） | 派发后验活：等汇报唤醒或验 worktree 落盘迹象；等待窗口预填 B1-B5 任务书静态段 |
| 23:02 | A-C1 290cfeb2：docs/design/service-registry-design.md 落盘（§0 三型逐字 + 10 闭集表 + services.json 格式 + 原子写/损坏拒读/同源同根 + 生效时机 + 3 备选舍弃） | B1/B2/B3 可逐字消费 |
| 23:03 | A-C2 47de0eec：gui-contract §20 服务总控（v19 加法，append-only 零改既有章） | B3/B4 消费锚点成文 |
| 23:05 | A-C3 ba0e5218：ipc-types §20 类型+签名桩（tauri invoke 桩/mock 显式报错桩编译伴随，tsc+既有 vitest 绿） | B4 类型基座就位 |
| 23:06 | A-C4 68b5a74f：i18n services.* 键块 zh-CN（中文初稿）+en-US 镜像，键名经 FlattenKeys 并入 I18nKey | B4 文案键清单=契约 |
| 23:07 | A-C5 4cac8821：authz 设计文末 Amended (2026-09-13) chat 接入判定（A-1~A-5，原文零改动） | B5 任务书可直接引用 A-1/A-2/A-4 |
| 23:13 | A-C6 4d649814：crates/p2p-service 桩（闭集表+双读 resolve+store 原子写/拒读，13 单测绿，一致性测试独立 registry_tests.rs 守 300 行红线） | B1 类型基座就位；workspace 经 glob 自动登记 |
| 23:14 | A-C7 e8012fd6：cli-parity.tsv service 段注释锚点（wsm-b3 去注释填充） | 守卫现行不红 |
| 23:16 | A 卡收尾门禁：fmt/line-limit/panic-hygiene 全 0；clippy（全 workspace）与 gui 全量 vitest 后台跑中，跑完 push feat/wsm-a | 等 clippy/vitest 结果 |
| 23:22 | A 卡门禁全绿：clippy workspace+src-tauri PASS(0)；gui vitest 234 文件/1431 测试全绿(0)；`git push origin feat/wsm-a` 成功（8 提交：7 交付 + 1 纯追加 skill 喂回）；合并回主干留主控（A 卡不合并） | 波次可转 B1-B5 派发；注意 i18n 实际路径 apps/gui/src/i18n（任务书写的 src/lib/i18n 为笔误） |
| 23:25 | 主控验收 A 卡 DONE：diff 16 文件纯加法 +953 零删除、契约三方锚定抽查逐字一致（设计 §1-3 / §20 / Amended A-1~A-2） | 验收通过 |
| 23:26 | 抢锁 ff-only 合并 feat/wsm-a → main @17176880；check-fast 首跑至 cargo test 段被主控 600s 超时截断（此前全段 PASS；合并树=A 已验证分支尖） | 门禁以证据为准：后台重跑 job bash-490 |
| 23:40 | A 卡收尾四步闭环：远端分支已删、worktree prune+残留目录已清、合并锁已释放 | A 会话暂留（B 卡契约问答源），波次收官时归档 |
| 23:41 | plan.md 升 B 波锁定：补 C1 接线卡、文件所有权细化（state.rs/daemon.rs→B1、Cargo 依赖行规则、B5 产出接线 diff） | 下一步：并行派发 B1-B5 |
| 23:44 | B 波五卡并行派发：B1=ecd95e49（registry 实装+装配+lanOnly）、B2=b419b14d（serve.* 三闸）、B3=fcc8dac5（CLI/tauri 命令+tsv）、B4=baff8188（服务面板前端）、B5=b3ec258d（chat PEP+auto-bind+回填） | 合并顺序 B1→B3→B2/B5→B4→C1；派发后验活凭 ledger 检查点行；C1 接线卡待 B1+B5 合并后派 |
| 23:57 | Ruling: B2 裁决点选 (a)——state.rs 禁触显式豁免 1 行（install 传 data-dir），独立提交+注明批准+rebase 到 B1 之后合并；否决 (b) 隐式中继 — 依赖顺序耦合是确定性隐患 — 错误代价：:121/:123 换序即静默失效。知会2 确认 B2 理解正确（Boolean 无条目=off=不装配），任务书「on（默认）」系主控措辞错误已认领。知会1 暴露主控遗漏：合并后未推 origin/main，后台补推中（job bash-500），完成前禁止各卡 rebase 旧远端 | B2 NEEDS_CONTEXT 解除，继续推进 |
| 00:01 | origin/main 同步完成（pre-push 快速门禁顺带全绿），main==origin/main==17176880；B 波验活：五 worktree 全活（B4 已落 1 提交 i18n 登记；B1/B2/B5 基线 17176880 正确；B3 旧基线 f12e0a9d 已发 [补充] 令其检查点边界 rebase） | 派发后验活通过；等各卡汇报 |
| 00:05 | Ruling: B5 题1=方案 A（trait 在 p2p-chat、判定核心 chat_admit 在 p2p-authz、newtype 壳在消费面：GUI chat.rs / CLI authz/）——p2p-authz 加 p2p-chat 依赖违反 §3 零业务依赖红线，B5 判断正确。题1b=允：c1-wiring.patch 触达面扩为 state.rs + state/chat.rs + daemon.rs 三文件 hunk（C1 逐字应用）。题2=附条件通过：Option gate 允许，但 None 时节点启动必须打 warn 可观测日志（chat authz 未接线=不设防），编译期强制列待细化区 — 依据：波次文件所有权使 C1 前无法编译期强制 — 错误代价：静默不设防违反失败路径可观测红线 | B5 NEEDS_CONTEXT 解除 |
| 00:44 | B4 五检查点落盘：C1 78abf777 i18n 注册（settings.nav.services + remoteAccess.serve.* 双语）/ C2 8121e8df mock services 内存实现（闭集+双读+损坏拒读，__MOCK_SERVICES__）/ C3 0e26b61d 服务总控卡（10 行渲染+乐观翻转+重启提示条+错误态，settings 页新增 services 分节）/ C4 d51dcc7a 两处类型修正 / C5 92d095d9 tunnel 被访启停控件（IpcBackend 两方法+卡片+mock-tunnel-serve，补 inventory 问题 2） | 截图证据链六张+console 落 shots-b4/；gui vitest 全量与 tsc -b 复验跑中（tsc --force 已过） |
| 00:55 | B3（fcc8dac5/wsm-b3）C1-C4 四提交落地：adaa7819 p2pctl service 域三命令（10 闭集实跑证据齐：list 来源列/enable-disable 落盘前后对比/闭集外附清单 Err/损坏·越闭集·版本不符显式 Err）→ a6930675 src-tauri services.rs（§20.3 逐字，242 行含 6 单测）→ 0aa21881 lib.rs 登记+p2p-service 依赖行 → 7197547c tsv service 段填充；门禁 parity(89 命令 73 映射)/line-limit/panic/fmt 全 0，clippy+gui-tauri 跑中，绿后 push+汇报 | 等 clippy 结果；merge 前置=B1（plan 顺序），base=origin/main@17176880 已 rebase |
| 01:11 | B2 (b419b14d) 五检查点落 feat/wsm-b2：llm_share 总闸 6b0b8229 / tunnel 持久化收编 fcb5f700（含批准的 state.rs 1 行豁免，commit 注明）/ a2a 启动闸 37f3f617 / cli offer unpublish 4601170f / docs 契约指南同步 e1ef858d | 提交拆分=三服务+CLI+docs |
| 01:14 | B2 全门禁绿：gui-tauri.sh / fmt.sh / clippy.sh（root+src-tauri+acp-agent+cli 手工段）/ line-limit / panic-hygiene / cli-parity（101 叶）/ ai-docs-sync（101 节 347 参数）全 0；skill 喂回 58338741；push origin feat/wsm-b2 完成（ls-remote 计数=1） | B2 DONE 待主控合并（序在 B1 后） |
| 01:15 | check-fast（bash-490）完成：34/35 段 PASS（含全量 cargo test、tauri、panic/protocol/parity/docs-sync），唯 gui-check 的 vite build 遭外部 SIGTERM（exit 143，与 B 卡并行构建争用相关，非断言失败） | 门禁不绿不视为绿：gui.sh 单独重跑中（job bash-557）；B 波合并窗口避开重跑时段 |
| 01:20 | B3 收官：四提交 push origin feat/wsm-b3 成功（adaa7819/a6930675/a9d0eed0/5f252268）；门禁全绿 cli-parity.sh=0(89 命令 73 映射)/fmt/clippy(root+src-tauri)/line-limit(478 文件)/panic-hygiene/gui-tauri.sh=0(16 套件，一次 clippy 并发压测致 control_page flake 已静默复跑证实)；apps/cli 170 测试+src-tauri services::6 测试全绿；STATUS=DONE 待主控按 B1→B3 合并 | 移交：lan_only 双读经 Value 直读 lanOnly 键与 B1 字段补齐解耦；CLI/tauri 胶水 ~40 行各面同语义实现（p2p-cli 不在本卡 scope），收敛建议归口 crates/p2p-cli::service 后续排 |
| 01:20 | B3 DONE 汇报（4 提交 push @5f252268，已 rebase 到 17176880）→ 主控机械验收通过：12 文件 +744/-3 全在 scope（main.rs 2 行注册已披露、依赖行符合规则）、services.rs 242≤300、tsv 两行 mapped 带理由、门禁证据齐全 | 验收通过·待合并（按序等 B1 先并）；移交三项入候选池：CLI/tauri 40 行胶水下沉 p2p-cli::service、tauri 运行态 requiresRestart 由 B4 联调覆盖、lanOnly serde Value 直读待与 B1 resolve 实现合并时对照 |
| 00:06 | B1 (ecd95e49/wsm-b1) C1 f08258de：p2p-service 节点装配开关解析 facade——NodeServiceSwitches.resolve（§2 双读两分支）+ load（§4.2 装配 fail-safe：读失败 warn 后按空表解析，显式报错面留 B3）+ 实装补强测试 3（save 建缺失 data-dir / §4.5 先读后写 upsert 合并 / 残留 .tmp 不污染）+ switches 双读/损坏/落盘覆盖 5 测试；A 桩 13 测试保持绿（21 全绿）；新增 tracing 依赖（fail-safe 告警留痕） | 签名零破坏，纯加法；Cargo.lock 随 C2 |
| 00:18 | B1 C2 97dfbfab：crates/p2p 装配消费——NodeConfig 增 ServiceSwitches（默认全 on 零行为变化），assembly 在 lan-only 剥离后消费：rendezvous_register=off 清 bootstrap 复用既有空表跳线路径（不注册不接线且不误触发无可路由告警）、relay=off 剥降级链、observe=off 跳出站观测（反射口不在域内）、rendezvous_server=off 不装配（public_only 原样透传）；p2p 依赖并 re-export p2p-service（apps/cli/Cargo.toml 归 B3，daemon 经 p2p::p2p_service 复用）；测试移出 assembly.rs（276 行守红线）+新增装配级 6 用例 | 关键取舍：CLI 无 p2p-service 依赖行额度 → facade re-export 解决，B3 胶水收敛候选可评估归此 |
| 01:19 | B1 C3 396bd932：lanOnly 断链修复——GuiConfig 补 lan_only（serde default false，契约 §16.5 落实）、config 默认+落盘重启回归、state 装配抽 node_build.rs（339→306 行）接 services.json 双读（根=app 数据目录与 authz 同源同根）、CLI daemon.rs 同规则（生效值回填 config 使 meta/status 反映生效配置）、src-tauri Cargo.toml 登记 p2p-service 依赖行（B2 复用）；src-tauri 5 处测试字面量机械补字段 | B2 注意：state.rs 的 install 段零改动，你的豁免 hunk 应无冲突；B3 的 Value 直读 lanOnly 与本卡 resolve 并存不冲突（B3 落条目、B1 装配消费） |
| 01:21 | B1 收官门禁全绿并 push：cargo test -p p2p-service(21)/-p p2p(17+集成)/gui-tauri.sh(0，143+16 套件)/p2pctl(158)；fmt/clippy(root+src-tauri)/line-limit(480 文件)/panic-hygiene 全 0；三个独立可 revert 提交 push origin feat/wsm-b1 @396bd932（基线 17176880 未动，合并留主控）；偏差披露：origin/main 当时落后本地 main，任务书「rebase origin/main」按基线=main@17176880 执行（origin/main 是其祖先，rebase 会丢 A 契约） | STATUS=DONE 待主控按序合并；注意 B2/B3 若在合并前 rebase 新 origin/main（含 A 契约推送）需各自确认无 Cargo.lock 冲突 |
| 01:22 | gui.sh 重跑 PASS（vitest 1431/1431，exit 0）→ 合并树 main @17176880 全量 check-fast 35/35 段绿，B 波合并基线门禁闭环。B3 第 5 提交为 skill 喂回纯追加（合规） | 基线可信；等 B1/B2/B4/B5 |
| 01:42 | B4 门禁收口 DONE：C6 b982363f lint 红改 effect 内联 IIFE 合规形态（offer-panel 先例）；gui vitest 全量 237 文件/1449 测试全绿（基线 1431+新增 18）、tsc -b exit 0、eslint PASS；`git push origin feat/wsm-b4` @b982363f（6 提交，基线 17176880）；截图证据链 shots-b4/ 6 张+console-clean.log（仅存量 acp discovery 取数 1 error，非 B4 代码路径） | STATUS=DONE 待主控验收；本机多会话高负载时段 app-boot/acp 域存量测试曾现超时假红（测试文件内 S4 注记已知负载模式），空载复跑即绿，代码无涉 |
| 01:43 | B1 DONE 验收合并：5 提交（f08258de..a2cf3a94）27 文件 +678/-142 scope 干净（仅 skill 喂回与机械 lock 连带）；§4.2 逐字核实装配 fail-safe 属契约口径（管理面拒读/装配按默认），B1 无偏差；ff-only 合并 main→a2cf3a94，锁正常收放，远端推送中（bash-575） | 顺序合并生效；B3 已令 rebase a2cf3a94 后回报再并；B1 会话留待收官归档 |
| 01:45 | B4 DONE 验收：6 提交 @b982363f（全 apps/gui/src/ scope 干净）、vitest 1449 绿（基线 1431+18 新增）、tsc/eslint 绿、6 截图+console-clean 齐全；面板截图亲核=§20.1 十项与型别/默认逐字一致；Minor：徽标文案「布尔阀」vs 契约「布尔型」入打磨池；存量 acp discovery console 噪声入候选池（问题 3/10 邻域） | 验收通过·待合并（序位 B4，B3 后）；B2/B5 在飞 |
| 01:52 | B5 (b3ec258d) 检查点就绪：C1 chat CheckGate 入站接线（红绿实测：红=未接线时 deny 用例挂起于 receive_media、接线后 3 handler 用例+6 闸单测绿；gate=None 装配 warn 按裁决附条件落地）+ 行数红线配套拆分 wire_envelope/media_in（line-limit 490 文件 PASS）/ C2 p2p-authz chat_admit（四因+ReadFailed 逐因测试，authz.denied 含 peer/perm/reason）/ C3 AutoBind 下沉共享+CLI default_role 改调（回归测试原样保留）/ C4 import_friends 幂等回填（重跑 0 新增/过期不改写/损坏表显式 Err）+ AuditKind::ImportFriends + p2pctl authz import friends + daemon startup_backfill 入口 / C5 GUI chat.rs 两入口 auto-bind+AuthzCheckGate 壳；c1-wiring.patch 产出（state.rs+state/chat.rs+daemon.rs 3 hunk）git apply --check @17176880 PASS | 测试/clippy/gui-tauri 门禁跑中，绿后 push feat/wsm-b5 并汇报；state.rs/daemon.rs 未触（patch 由 C1 应用） |
| 02:08 | B3 收尾 rebase 完成：rebase onto a2cf3a94（冲突 3 处均平凡：双 Cargo.lock 取侧重生成、skill 喂回双追加并集合并）；对齐 B1——p2p-service::switches 确认为装配面（命令面无交叠），services.rs 收编型回落改 typed GuiConfig.lan_only（删 Value 直读，232 行）；gui Cargo.toml 重复依赖行按规则消重；门禁复绿 parity=0/fmt=0/limit=0/panic=0/p2p-service 21+cli 170+src-tauri 16 套件/clippy(gui)=0；7 提交 force-with-lease 推送 fee2f878，STATUS=DONE 待 ff-only | 无 |
| 02:08 | B3 收尾验收合并：rebase a2cf3a94 干净重放+语义对齐（lanOnly 改 typed、依赖行消重），门禁复绿全套；BASE-OK 后 ff-only 合并 main→fee2f878，远端已推（bash-585）；B1/B3 worktree/分支后台清理中（bash-587）；B4 已令预 rebase（其 worktree 见新提交 ee07f911）；B2 有产出（58338741）、B5 仍实现中 | 队列剩 B2/B5→B4(rebase 中)→C1 |
| 02:13 | B4 预 rebase 完成：origin/main @a2cf3a94（fee2f878+skill 追加）上 6 提交干净重放零冲突（b982363f→ee07f911），与 B3 services.rs 命令名逐字相认、与 B1 typed GuiConfig 无前端涉；tsc -b exit 0，聚焦 vitest settings/remote-access/mock-services/guard 全过（app-boot 隔离 14/14，全量批跑中一次文件级负载假红），force-with-lease push 完成 | B4 可按队列 ff-only；B4 侧无遗留 |
| 02:13 | B4 验收合并：rebase 实际已落 fee2f878（merge-base==main 实证），ff-only 合并 main→ee07f911（18 文件 +1103/-19 全 apps/gui/src/），远端推送与 worktree/分支清理后台进行（bash-588/589） | 队列剩 B2/B5→C1；B2/B5 收尾 rebase 到 ee07f911 |
| 02:47 | B5 收官 push：7 提交 @83a78135 push origin feat/wsm-b5（rebase 到 ee07f911=B1+B3+B4 已并主干，零冲突）；门禁全绿：cargo test p2p-chat(54+3 集成)/p2p-authz(61)、apps/cli 170、gui-tauri-check、fmt、clippy（workspace+src-tauri，-D warnings）、line-limit(492)、panic-hygiene、protocol-registry、cli-parity(102 叶)、ai-docs-sync(102 条目 349 参数)；scope 核对 23 文件全在 B5 所有权内（state.rs/daemon.rs 零触达）；偏差披露：①CLI startup_backfill 观测沿 daemon eprintln 口径（apps/cli 无 tracing 依赖，Cargo.toml 归 B3 不动）②GUI 三接线符号 allow(dead_code) 标注（C1 patch 应用前无调用方）③p2pctl chat serve / chat 命令上下文仍无闸（不在本波接线清单，gate=None warn 兜底） | STATUS=DONE 待主控验收；C1 应用 patch 后建议保留 allow(dead_code) 注释（应用后自然生效，注释即文档） |
| 02:47 | B5 DONE 验收合并：7 提交 @83a78135（BASE-OK 实证 rebase 到 ee07f911 零冲突），23 文件 +1410/-298 全 scope，红绿双向+幂等回填+审计证据齐全；ff-only 合并 main→83a78135；远端推送（bash-606）与 B5 worktree/分支清理（bash-607）后台进行；B5 移交④「其余 chat 装配点接线」入候选池 | C1 接线卡已派 session-0fd30b55（patch 平移+B1 后实况对齐）；在飞剩 B2+C1 |
| 03:21 | C1 DONE 验收合并：2 提交 @286f2441（BASE-OK，5 文件 +13/-7 含披露的 pub mod 编译连带），ff-only 合并；chat PEP+启动回填 GUI/CLI 主链路生效；远端推送（bg）与清理（bg）进行中 | C1 移交两条入候选池：①apps/cli 存量 fmt 漂移+fmt.sh 未覆盖该域 ②chat serve/context 装配点 gate=None（=B5 移交④）。在飞仅剩 B2 |
