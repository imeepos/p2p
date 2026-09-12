# Orchestrator 账本 — DSH Web over p2p 隧道波（2026-09-11）

计划：`.orchestrator/2026-09-11-dsh-tunnel/proposal.md`（含用户裁决）
协调会话：session-3373f897-f9c0-4a32-9af2-b9562eecc311
基线：main @ 4df3eb5（== origin/main）

## 任务

- W-T1 tunnel/1 规范页 + 三处登记 + gui-contract §19：dispatched（sessionId=session-b39c3e8b-4943-4d0e-aa6b-b2b97e89a224）
  分支 docs/wt1-tunnel-spec / worktree .worktrees/wt1-spec / 4df3eb5 / 任务书 brief-wt1.md
- W-T2 被访侧 p2p-tunnel crate + itest：dispatched（sessionId=session-a7b1fd9f-360f-4865-9b98-6243e14252b8）
  分支 feat/wt2-tunnel-responder / worktree .worktrees/wt2-responder / 4df3eb5 / 任务书 brief-wt2.md
- W-T3 访侧反代 + GUI 远程访问入口：dispatched（sessionId=session-0d91cd06-84b8-4f5e-a800-1d8a37bf89a7）
  分支 feat/wt3-tunnel-client / worktree .worktrees/wt3-client / 4df3eb5 / 任务书 brief-wt3.md

## Facts（已验证，附证据）

- F1 DSH Web = 原生 node:http，默认 127.0.0.1:3080；`--host 0.0.0.0` 被 CLI 显式拒绝；无 TLS/认证/来源策略（源文件行号见两份调研报告）
- F2 鉴权两层：进程内一次性 token（重启失效）→ HMAC cookie；**cookie 名与 payload.authority 均绑定请求 Host**；`/api` 另有 Host/Origin 围栏
- F3 前端后端地址全靠 `location.origin` 同源推导；`__DSH_BOOT__` 不含地址端口；`<base href="/">` 强制站点根
- F4 DSH **不设 X-Frame-Options/frame-ancestors**（grep 无命中）→ 未来内嵌视图无障碍
- F5 p2p 仓无任何通用 HTTP 代理/隧道；协议注册表 16 条全 implemented，无隧道类占位
- F6 可复用接缝：协议 handler 注册缝 / acp-pump 的「本地回环服务+token+一连接一泵」/ control 的 endpoint+token 发现范式 / 帧封装与 chunked 设施
- F7 验收环境 102 可达（ssh 免密 OK），其上 DSH 0.1.0-rc.6 + node v24.18.0；**已存在 p2p-bridge 与 p2p-bridge-rc32 数据目录、start-p2p-rc32.sh 指向 `dsh --profile web --patch /tmp/p2p-102-rc32.patch.yml --port 4993`**；
  但 `/tmp/p2p-102-rc32.patch.yml` 与运行日志当前**不存在**（该实例未在跑或已被清理）
- F8 事故：`p2p/AGENTS.md` 被覆盖成 `/Users/imeepos/ext512/cms/AGENTS.md` 的逐字内容（跨仓误写）；协调者已 `git checkout -- AGENTS.md` 还原，工作树恢复干净

## Guesses（带过期条件）

- G1 W-T3 的 Host 重写能同时满足 cookie authority 与 Host 围栏 —— 证伪条件：端到端实测仍 403/401（此时按 NEEDS_CONTEXT 回报，重审契约）
- G2 底座 1 MiB 帧上限 + 64 KiB 出站分块足以承载 DSH 的 SPA/插件 bundle/流式增量 —— 证伪条件：实测出现帧超限或明显卡顿（此时改分块参数，属波次内细化）

## Rulings

- Ruling: 采用"访侧本地回环反代 + Host 重写"为唯一形态，不做 trusted-host/子路径路线 — 依据：F2/F3 四条约束（cookie 绑定、Host 围栏、同源推导、根路径） — 错了的代价：需改 DSH 侧鉴权模型（本轮已裁决 DSH 零改动，故不可行）
- Ruling: 契约由协调者冻结并随任务书逐字下发，三卡并行 — 依据：并行效率原则"契约钉主干"（下游只消费签名，冻结后即可并行） — 错了的代价：若契约有误，三卡同错（故 W-T2/W-T3 均被要求"发现契约不可行先报 BLOCKED，禁自行改义"）
- Ruling: W-T2 与 W-T3 在同一文件 `apps/gui/src-tauri/src/lib.rs` 命令表追加行，位置与顺序写死（W-T2 两行在前、W-T3 两行在后，插入点 `authz_default_role_save` 之后） — 依据：并行批次禁共用单根文件是理想，此处不可拆，故改为预分配精确插入点 — 错了的代价：仍会有机械冲突，但 rebase 可无损消化
- Ruling: 本轮 DSH 仓库零改动（用户裁决） — 依据：避免跨仓耦合与 DSH 发布依赖 — 错了的代价：token 重启失效需人工重贴，体验差（登记为后续波次候选）

## 进度账本（每事件一行）

- 2026-09-11 12:57 事件：三卡派发完成（五问：完成【否，刚派】；空转【否】；进展【契约冻结并落盘、三 worktree 基线就绪】；下一步【三卡执行，我并行处理在途 W1/W2 的汇报与合并】；指令【见三份 brief】）
- 2026-09-11 12:41 事件：AGENTS.md 跨仓误写（已还原）+ 用户委派新需求评估（未派发，先调研）
- 2026-09-11 12:52 事件：用户裁决形态 A / 人工粘贴 token / 本机+102 验收

## 待细化区（本波之后）

- DSH 内嵌视图（第二里程碑，已确认无 X-Frame-Options）
- token 生命周期治理（DSH 侧插件或外壳挂钩，用户已选"本波零改动"）
- `~/.dsh/dshplug` 两个已构建插件（host-p2p-bridge rc.32 / client-ui-p2p rc.1）能力核对：它们与 102 上已存在的 p2p-bridge 数据目录同族，可能存在重叠能力，需在下一波开工前先做一次只读能力对照

## Handoffs

- 无（本轮全程本会话）
- 2026-09-11 13:04 事件：W2 汇报 DONE → 我侧独立复核通过（ACCEPTED，未合 main）
  （五问：完成【是，验收证据齐】；空转【否】；进展【docs/protocol 七文件 +135/−5 落地，三门禁我逐条重跑 EXIT=0；
  registry 仅 vectors 数组追加一行】；下一步【等 W1 合入并全量 make check 绿后再 ff 合入 W2，之后清 worktree/分支/归档】；
  指令【已 send：冻结待命，勿动 worktree、勿新开提交】）
  - 复核证据：git diff --stat 仅 docs/protocol/**；rendezvous.rs:213-217 代码实况与文档一致；
    protocol-registry EXIT=0（456 文件/16 id）、line-limit EXIT=0（448 .rs）、ai-docs-sync EXIT=0
  - Ruling: spec-charter §8 首波清单不追加新向量文件名 — 依据：章程属契约面改动，非本卡范围且在途不应扩面 — 错了的代价：章程与向量目录短期不完全一致（已记待细化区）
- 2026-09-11 13:19 事件：W-T1 汇报 DONE → 我侧复核 ACCEPTED（主体），并发现**契约缺口**已回退补件
  （五问：完成【主体是，缺口否】；空转【否】；进展【specs/tunnel.md 191 行 + registry 第 17 条 + design/wire-protocol §3.2 行 + gui-contract §19 落地；
  registry 门禁我重跑 EXIT=0；merge-tree 干跑 W-T1×W2 无冲突】；下一步【令 W-T1 补 §19 被访侧开关两命令，补完再统一合并】；
  指令【已 send：补 tunnel_serve_start/stop + TunnelServeStatus，只动 gui-contract】）
  - 缺口定性：§19 只登记访侧两命令，被访侧无开关命令面 → 产品里无人能加白名单/开启服务，准入门永远进不去
  - 路径偏差定性：**我的任务书写错**（docs/protocol/wire-protocol.md 不存在，真值源 docs/design/wire-protocol.md），执行者按实际落位正确
  - Ruling: 三处未定语义采纳 W-T1 写法（白名单未命中/未开启→target_not_allowed；bytes_in/out 按记录方视角；访侧 ack 超时值不冻结）
  - Ruling: tunnel_status 定为访侧会话快照 + 独立 Tauri 事件（不动 NodeEventJson 判别联合）— 依据：避免在节点通用事件通道混入高频状态 — 代价：访侧/被访侧状态分两处看
- 2026-09-11 13:22 事件：main 前进 4df3eb5→a7208ef（skill 教训沉淀，独立 worktree 全流程：建树→提交 a7208ef→push→ff→清树删分支）
  - 内容：派发任务书前必须先 ls/glob 验证路径（本卡实证我把 wire-protocol.md 路径写错）；慢门禁（ai-docs-sync 冷构建约 5 分钟）任务书里预先标注
  - 副作用：全部在途分支基线落后，已通知 W-T2/W-T3 收尾时 rebase；W-T1/W2 合并前 rebase
- 2026-09-11 13:26 事件：W-T1 完工 tip=12fb932e（5 提交）→ **验收通过并冻结待命**
  （复核：diff vs main 恰 4 文件 +287 行；末提交仅 1 行 §8 同步；main 仍是祖先 FF-OK；registry 门禁 17 id PASS）
  （五问：完成【是】；空转【否】；进展【契约面全部落地：规范页/registry/design wire-protocol §3.2/gui-contract §19 四条命令+事件】；
  下一步【排入合并队列，顺序 W1 → W-T1/W2】；指令【已 send：冻结待命，勿动 worktree】）
- 2026-09-11 13:26 事件：W1 根因定位（读其 PROGRESS.md，未打扰执行）
  - 根因：**bash 在多字节 locale 下把 `$var` 后紧跟的非 ASCII 字符并入变量名**（`$name（` → 变量 name+全角括号），set -u 击杀；
    C locale 不触发 → 解释"本机绿、CI 三连红且我本地也复现（我跑时 LANG 未设/CI=true 组合触发）"
  - 红链：affected.sh:123 回退 err 被杀 → KEY=VALUE 缺失 → 断言真实失败 → affected-fast.sh:32 报红行自身被杀（FAIL 被吞）→ unbound → make Error 1
  - 修复：6 处 `${}` 括护（affected.sh/affected-fast.sh/protocol-registry.sh）+ 新增 `scripts/check/tests/ascii-var-guard.sh`（LC_ALL=C 字节级扫描，挂 gate-tests 首位）
  - 我侧独立抽检：守卫脚本已读，pattern `\$[A-Za-z_][A-Za-z0-9_]*[^[:print:][:space:]]` 与注释机理自洽，`hits=0` 即绿
  - 待其完成：全量绿 ×2 + 注入红/还原绿 + rebase + push
- 2026-09-11 13:41 事件：**我自己的复现条件更正**（自查发现，非子会话问题）
  - 事实：`CI=true make check`（不设 locale）后台跑完 **MAKE_CHECK_EXIT=0**（/tmp/local_make_check2.log:5565，5565 行日志全绿）
    → 我最初给 W1 的"改前必红"命令不含 locale 条件，属**我任务书的事实错误**
  - 正确触发：`LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 bash scripts/check/tests/affected-fast.sh` → exit 1；bash 5.3.9 亦复现
  - 处置：已 send 更正 W1（要求红绿证据逐字带 locale）+ 原样追加到 brief-w1.md 附录（事实源与任务书一致）
  - 附带发现：用 W1 守卫 pattern 全仓扫描，`scripts/**` 下另有约 10 个 ops/release/deploy 脚本存在同类"失败分支哑弹"
    （绿路径不求值故不红，真失败时吞错误行）→ 本轮不扩范围，等 W1 汇报后裁决是否立新卡
- 2026-09-11 14:30 事件：W1 汇报 DONE（tip e1650a80，已 rebase main）→ 我侧复核**通过**，末道主干全量门禁后台运行中
  - 我方独立证据（不照抄自报）：①守卫注入实验：往 release-gates.sh 追加 `$HOME（注入）` 探针 → hits=1 精确定位注入行 EXIT=1；还原 → hits=0 EXIT=0
    ②locale 矩阵：修复后 affected-fast 在 UTF-8 与 C locale 均 EXIT=0 pass=20；主树旧代码对照复现 `line 32: name<乱码>` EXIT=1
    ③第二卡口旁证：W1 worktree 内 acp-echo-stub 时间戳 22:32（晚于建树）→ test.sh 冷构建分支真执行过
  - 第二卡口（W1 绿1轮暴露）：apps/acp-agent 被 exclude 在根 workspace，cargo test --workspace 永不构建 acp-echo-stub
    → p2p-itest task-wave 在 fresh 环境 6/6 秒红；主树靠 9/9 历史残留侥幸绿。修法=test.sh 自保障夹具（仿 cli-parity.sh 先例）
  - Ruling: ci.yml 的 rust-cache 加 apps/acp-agent（CI 冷构建 +3~6min）→ **本轮不改 workflow**，立后续卡
  - Ruling: guard 覆盖 githooks/** + scripts 下约 10 个 ops/release/deploy 脚本的同类"失败分支哑弹" → 立后续卡（发现者 W1 记功）
- 2026-09-11 14:31 事件：main 前进 a7208ef→00b19568（W1 双卡口教训入 self-evolving：locale 变量名炸弹 + 夹具自保障 + 「仅某编排复现」先对齐环境全差）
  - 独立 worktree 全流程执行完毕（建树→提交 00b19568→push→ff→清树删分支，远端分支同删）
  - 副作用：W1 分支基线 a7208ef 落后一笔纯 docs；合并时以 rebase 后为准
- 2026-09-11 14:31 观测：W-T3 已有提交（24dffa9a），W-T1 12fb932e 冻结，W-T2 仍 4df3eb57（未见提交，PROGRESS 1597B 在位）
- 2026-09-11 14:42 事件：**W1 合入 main**（独立验收完成）
  - 全量门禁证据：我在 W1 worktree `e1650a80` 后台跑 `CI=true LANG/LC_ALL=en_US.UTF-8 make check` → **ACC_MAKE_CHECK_EXIT=0**（/tmp/acc_w1_full.log，5557 行）
  - rebase 到 00b19568 后 tip 变 46384ba3（哈希重写，`--force-with-lease` 推送）；rebase 后 `make gate-tests` 复验 EXIT=0
  - 合并：main 00b19568 → **46384ba3**（ff-only），已 push origin
- 2026-09-11 14:43 事件：**W-T1 合入 main**（契约面全落地）
  - rebase main 后 tip 12fb932e→1ed36744（5 提交重放），registry 门禁复跑 PASS（456 文件/17 id），`--force-with-lease` 推送
  - 合并：main 46384ba3 → **1ed36744**（ff-only，4 文件 +287 行），已 push origin
- 2026-09-11 14:43 事件：main 全量门禁启动（后台 /tmp/main_check_after_w1_wt1.log）——覆盖 W1+W-T1 合并后状态，绿则清 W1/W-T1 worktree 与分支并归档两会话
  - 注：W1 的 `scripts/check/test.sh` 夹具自保障改动已进 main，后续所有卡的 `make check` 自动受益
- 2026-09-11 14:43 事件：W-T2 汇报 DONE（tip 636c280f，已 rebase main 7e8576c6 之上）
  - 我侧独立复核：单测 14 passed / itest tunnel_wave 7 passed（均 EXIT=0，重跑）；六码错误帧+审计实证收讫；
    注入实验×2 认可；lib.rs 插入位置合规；MAX_TICKET_BYTES=4096 / CHUNK=64KiB / ts 窗口与契约一致
  - descriptor_tests.rs 9 行纯格式化 → 采纳（暴露 apps/acp-common 在 fmt 门禁外，记后续卡）
  - **新发现的疑似契约破坏**：W-T2 自报「Node facade 双写协议 ID 帧」（new_stream 一帧 + open_with_protocol 一帧，
    handler 侧余一帧需 skip）→ 若发生在**我方拨出**方向，第三方 responder 按规范实现必翻车（mininode 同款问题反向）
  - 已要 W-T2 三样证据（调用点/帧级实证/影响面 grep），拿到后裁决是否立"facade 去重写"修复卡
  - Guess G3：facade 出站流双写协议 ID 帧且既有 handler 靠 skip 容差 — 证伪条件：W-T2 证据显示只影响特定 API 组合或方向
- 2026-09-11 15:44 事件：main 终验（7e8576c6）EXIT=2 → 定性**负载型 vitest 假红**，非代码缺陷
  - 证据：229 测试文件/1399 用例全 passed，唯一 error = `Failed to start forks worker ... Timeout waiting for worker to respond`
    （contacts-page worker 未启动）——与 IM-T43 固化的签名逐字同族
  - 背景：当时机器连跑两次全量 make check + 三个子会话 worktree 并行构建（1 小时前同内容全量跑 EXIT=0）
  - 处置：负载回落（load 2.91）后隔离复跑 `bash scripts/check/gui.sh`（后台 /tmp/acc_gui_rerun2.log）；**会话归档押后到复跑绿**
  - Ruling: W2 的 docs-only 改动与此红无关 — 依据：vitest 面不吃 docs 输入，且上轮同内容已绿 — 代价：归档推迟约 15 分钟
- 2026-09-11 15:55 事件：三卡现场清理启动（W1/W-T1/W2 worktree+本地/远端分支）
  - 首次前台删除在 W1 的 GB 级 target/ IO 上超时被杀（worktree 已 prunable、目录残半）→ 改后台 rm -rf + prune + 删分支（bash-85）
  - 教训候选：`git worktree remove` 对含全量构建产物的树要用后台 + 显式 `rm -rf`，前台 5 分钟上限必撞
- 2026-09-11 16:04 事件：gui 隔离复跑 **EXIT=0**（230 文件/1404 用例全过）→ 上一轮 EXIT=2 的负载型假红定性坐实
  （文件数 229→230 吻合：上轮 worker 未启动的 contacts-page 本次正常计入）。main 7e8576c6 门禁验证完整。
- 2026-09-11 16:05 事件：**W-T2 合入 main**（636c280f，ff + push）
  - 我侧合并前门禁：clippy -D warnings EXIT=0（11.4s 暖缓存）/ panic-hygiene PASS（216 文件）/ line-limit PASS（459 文件，新 crate 最大 296 行）
  - 合并后全量 make check 后台运行中（/tmp/main_check_after_wt2.log）；W-T2 worktree/分支清理后台进行中（bash-87）
- 2026-09-11 16:06 事件：**facade 双写裁决**（W-T2 证据三件套：调用点 node.rs:98-107 + streams.rs:17 契约、帧级日志
  /tmp/wt2-itest3.log、影响面 grep 全仓仅 2 处容差）
  - Ruling: 采纳方案 b —— 规范维持单帧口径不改；工厂契约注释补"禁止内嵌 open_with_protocol"；W-T2/llm-share 的 skip 容差保留（混合版本兼容）；
    立 **W-T4 修复卡**（新专属会话，排 W-T3 合入后串行）：清扫全部"包了 Node::new_stream 的 StreamFactory 装配"
    （itest NodeFactory / llm-share borrow_dial / gui 访侧装配若有），红绿 itest 用无容差严格 responder 锁死
  - 依据：方案 a（facade 去重写）改所有 Node::new_stream 消费者语义，契约相邻面风险大收益小；方案 b 切口小、修复真实的互操作缺陷
    （0.1.7 的 llm-share 拨号对严格第三方 lender 同样翻车）
  - 已给 W-T3 发装配红线：访侧工厂只准包裸 SwarmFactory（Node::request 先例），并要求其自验断言"首业务帧=票据 JSON"
- 2026-09-11 16:06 状态：W1/W-T1/W2 三会话可归档（内容全落地+主干验证完整，归档为宿主操作待用户/后续执行）
- 2026-09-11 16:08 事件：W-T2 确认裁决 + 交结构化复盘（归档材料，收档如下）
  - 做对了：红先实证（依赖缺失 exit 101）；注入实验红→绿；pump JoinHandle 完成后禁再 poll；大流量读写须并发（先写后读在管道缓冲<1MiB 时自锁）
  - 踩坑：①双写 ID 帧最初当玄学 bad_ticket——"先对工厂契约再怀疑 handler"；②tokio duplex 64KiB×链 + mux 窗口总缓冲小于请求量时
    "写完再读"测试必死锁，测试须模拟真实反代并发拷贝；③mac 无 timeout，长测走后台 job+日志
  - W-T4 起点清单（随卡交付）：apps/cli/src/llm_share/borrow_dial.rs:134 + crates/p2p-itest/tests/tunnel_common/mod.rs NodeFactory
- 2026-09-11 17:13 事件：W-T2 合并后全量门禁 **clippy 红**（src-tauri doc_lazy_continuation，tunnel.rs:8 列表项延续行缺缩进）
  - 定责：**验收盲区，双方各半** —— W-T2 任务书 GUI 侧自验只写 cargo check -p p2p-console（未含 src-tauri clippy）；
    协调者复核口径镜像了任务书（只验了新 crate clippy），同样漏掉 —— 任务书缺陷是根因
  - 处置：紧急热修 cf6b2532（一行文档注释分段）→ src-tauri clippy 复验 EXIT=0 → push → 全量重跑后台
  - 规则固化：**凡触 apps/gui/src-tauri 的卡，验收必含 `cargo clippy --all-targets -- -D warnings`**（W-T3 任务书已有此条，后续任务书沿用）

## 2026-09-11 全量门禁第二红：cli-parity 缺映射（契约缺口，协调者修）
- bash-88 全量 make check @ cf6b2532：clippy 过后死在 cli-parity——
  tunnel_serve_start/stop 在 generate_handler! 有行、cli-parity.tsv 无行（EXIT=2）。
- 定责：契约缺口。gui-contract §19 冻结时漏写 CLI 对等条款；W-T2 任务书验收面未含
  cli-parity（同 clippy 缺口同根因：任务书只写了 cargo check -p p2p-console）。
  协调者复核镜像任务书口径，未兜住。教训同前：任务书必须含「全量门禁清单或明确
  豁免」；协调者复核不镜像任务书面，按 make check 全链口径。
- 修复（worktree fix-parity，分支 fix/wt2-cli-parity）：
  - 9599fb14：cli-parity.tsv +2 exempt 行（serve 生命周期绑 GUI 常驻节点，无 CLI
    常驻面，llm_share_serve_status 先例）+ gui-contract §19 补第 8 条 CLI 对等条款
    （四条命令均 exempt；open_dsh/status 随 W-T3 同卡加行）。
  - 2e53b4dd：registry.toml tunnel/1 planned→implemented、draft→frozen（/im/chat/1
    先例）+ wire-protocol §3.2 措辞同步（欠账清偿）。
- 规则固化：凡新增 GUI 命令的卡，验收必含 cli-parity + ai-docs-sync（有 CLI 对等面
  则连 p2pctl 子命令与 ai-guide 条目一起交）；任务书模板补该条。
- W-T3 必须随卡给 cli-parity.tsv 加 tunnel_open_dsh/tunnel_status 两行 exempt
  （理由引 §19.8），否则其合并后主树必红同款。
- 验证与合并：worktree 内 cli-parity（CLI-PARITY-OK，GUI 80 命令/映射 69/豁免 11）
  + protocol-registry PASS + ai-docs-sync AI-DOCS-OK 三绿；main==origin/main 基线核对
  后 ff-only 合并，主树 @ 2e53b4dd 已推（pre-push 快速门禁两轮 PASS）。收尾清理与
  全量 make check 终认证后台进行中。

## 2026-09-11 主树全绿认证 @ 2e53b4dd
- 全量 make check（CI 等效 locale）EXIT=0，十二道门禁完整走通首次全绿：
  gate-tests/version/fmt/line-limit/clippy/test/gui-check/gui-tauri-check/
  panic-hygiene(216 文件零 unwrap)/protocol-registry(17 id)/cli-parity(80 命令
  映射 69 豁免 11)/ai-docs-sync(99 条目 332 参数 6 示例)。
- 覆盖范围：W1 + W-T1 + W2 + W-T2 + 协调者两笔热修（cf6b2532 clippy、
  9599fb14+2e53b4dd cli-parity/registry）。W1/W-T1/W2/W-T2 四会话产物稳定，
  可交用户归档。
- 在途：仅 feat/wt3-tunnel-client（已注入 cli-parity 加行要求）。

## 2026-09-11 W-T3 中期进度（合规，已放行）
- 已 rebase 新 main（W-T2 已在内），lib.rs 四行命令表按约定序并存。
- 删自研 client.rs/ticket.rs，改消费 p2p_tunnel 导出（TunnelClient/TunnelTicket/
  TunnelErrorCode/PROTOCOL_ID），零重复实现。
- 装配方案（对 W-T4 任务书有影响）：W-T3 新增 Node::open_raw_stream 出裸流
  （不写协议帧），访侧唯一写帧点=TunnelClient——即 crates/p2p 将同时存在
  new_stream（包装流工厂）与 open_raw_stream（裸流）两个原语。W-T4 的
  double-write 清扫 brief 编写时按此更新：迁移目标是 bare SwarmFactory 或
  open_raw_stream 二选一，等 W-T3 落地后看其用法再定，避免 brief 过时。
- 自测三坑已修（spawn 前求值死锁/对端帧面 IO/帧界容忍），21/23 绿，流式用例
  验证中；随后 cli-parity +2 exempt 行→全门禁→push。

## 2026-09-11 W-T3 DONE_WITH_CONCERNS 接收与独立复核
- 分支 feat/wt3-tunnel-client @ e7229f7a，rebase 于 origin/main 头 2e53b4dd（已核实
  merge-base 一致）。改动 24 文件 +2018 行，全部 ≤300 行/函数 ≤60 行。
- 协调者红线复核四项全过：①open_raw_stream 裸包 SwarmFactory，注释写明单写帧点
  依据（引 2026-09-11 裁决）；②lib.rs 两段路径字面 tunnel::tunnel_open_dsh/status
  且四行并存；③cli-parity.tsv tunnel_open_dsh/status 两行 exempt 引 §19.8；
  ④Host 重写与 URL 解析严格 127.0.0.1 字面量（head.rs/url.rs 有正反测试）。
  W-T4 禁区（borrow_dial.rs、itest tunnel_common）零触碰。
- 自报验收：pnpm lint/test(1408)/build/check:i18n 绿；cargo test -p p2p-console +
  clippy 双 crate -D warnings 绿；CLI-PARITY-OK；panic-hygiene PASS。
  协调者全量 make check 独立复核进行中（worktree wt3-verify）。
- 已知缺口（W-T3 自报，如实）：GUI 全链 e2e 实证未完成（隔离 dsh web 实例可起、
  boot URL 捕获、错误 token 401 已实测；双 GUI 互连+serve_start+浏览器截图链缺）。
  G1 疑点消除一半：真实 DSH 303 set-cookie authority=127.0.0.1:3080 实测可见，
  cookie host-only 与端口无关，Host 重写方案与 cookie 机制兼容。
- 移交 W-T4 的新增 MUST：pump 半关时序真实 TCP wire 复核（W-T3 测试实证「FIN 后
  对端写帧 pump 读端 0 字节」疑点，产品路径暂用 duplex wire 测试覆盖，需红绿
  双向定案）。
- 遗留待用户裁定：e2e 补做方式（派 W-T3b 小卡 / 先补后合 / 用户手工验证）。

## 2026-09-11 用户裁定：W-T3 先合并、再派 W-T3b 补 e2e
- 用户选定方案 A：协调者独立全量门禁绿后即合并入主干；随即派 W-T3b 证据小卡
  （预算封顶 120 调用）补双 GUI 互连→serve 开关→浏览器全链截图证据链，证据齐后
  补记 W-T3+W-T3b 合并验收裁定。任务书 brief-wt3b.md 已就绪。

## 2026-09-11 W-T3 合并入主 + W-T3b 派发
- 独立复核：wt3-verify worktree 全量 make check EXIT=0（GUI 82 命令/映射 69/豁免 13，
  AI-DOCS-OK）。main==origin/main 基线核对后 ff-only 合并 feat/wt3-tunnel-client，
  主树 @ e7229f7a 已推（pre-push 快速门禁 PASS）。
- W-T3 未提交的 PROGRESS.md（44 行调试记录）已抢救存档至
  .orchestrator/2026-09-11-dsh-tunnel/wt3-PROGRESS.md；两 worktree 与分支清理后台进行。
- W-T3b 已派发：新会话 session-351db2f8-5472-44a5-b8a0-341a4373a769，任务书
  brief-wt3b.md（预算封顶 120 调用，零代码改动，九项证据链），命其先读
  wt3-PROGRESS.md 复用调试结论，第一步确认 tauri 单实例锁/双实例隔离机制。
- 验收裁定：W-T3 代码面 PASS（本日门禁证据链完整）；e2e 实证项挂 W-T3b 待补，
  补齐后合并裁定。

## 2026-09-11 W-T3b 阶段0 PASS（实例隔离机制确认）
- 无单实例锁（tauri 仅四插件）；P2P_CONTROL_PORT env 钩子寻址；app_data_dir 按
  identifier 固定不可分（endpoint.json 后写覆写为已知限制）；节点身份隔离走
  settings.saveAndRestart(dataDir=/tmp/wt3b-node-a|b)；quic/tcp/tunnel 反代全临时端口。
- 发现缺口（只记录）：remote-access 路由不在控制通道 ROUTES 11 条白名单与
  page-registry → tunnel_open_dsh/serve_start 无法经控制通道驱动。
- 【新增 backlog 卡】remote-access 页接入控制通道 page-registry/ROUTES：
  补 headless 驱动面，未来 e2e 可脚本化，免 AX 点击；与 §19 契约不冲突
  （控制通道面由 gui-control-channel.md 治理，不动 generate_handler!）。
- W-T3b 转 AX 探针驱动真 GUI（PID 区分双实例），预算 26/120。

## 2026-09-11 W-T3b 预算预警与超限批准
- 111/120 时按红线预警。阶段成果：双 GUI 实例跑通（A=5VBVBUpk… node-a、B=2VMiV9DW…
  node-b，peerId 互异均 running）、DSH 隔离实例 boot URL 符合契约形状、控制通道
  /page/action 驱动 settings/peers 实测成功。
- AX 被 TCC 拒（osascript not allowed assistive access）。应对：HTTP 注入式驱动
  （vite 移 5174、5173 反代注入 driver.js，webview 内走真实 __TAURI_INTERNALS__.invoke
  + DOM 驱真实表单），零仓库改动，注入链已验证（标记=1）。
- 协调者批准超限：上限放宽至硬顶 140；到顶仍缺硬证据（首屏+WS 流式对话）即停，
  写 DONE_WITH_CONCERNS 归档。驱动链终报须完整披露以复核证据效力。
- 再次确认 backlog：remote-access 控制通道注册卡（本卡驱动缺口的根治面）。

## 2026-09-11 W-T3b 终报裁定：DONE_WITH_CONCERNS 接收
- 证据链：1/2/6/7/8 PASS；3/5 半 PASS；4 FAIL（硬项）。全档
  .orchestrator/2026-09-11-dsh-tunnel/wt3b/EVIDENCE.md（协调者已抽查，实质充分）。
- **真缺陷定案（判别实验证实）**：head.rs 反代只重写 Host、未重写 Origin/Referer →
  浏览器 Origin=127.0.0.1:<proxyPort> 被 DSH /api authority 栅栏 403；GET 过、
  POST/WS 拒；直连 3080 全 200。修复域=p2p 侧 head.rs 重写面扩展（合形态 A
  「DSH 零改动」裁定），非 DSH 侧问题——G1/Origin 疑点至此全部闭环。
- 观察点记录（非隧道域）：节点 swarm TCP 监听 *:52056/52080 通配；控制通道
  /screenshot 帧源滞后（三帧同字节，两张证据截图同尺寸即此故，DOM 文本取证可靠）；
  错误态与已开启卡并存展示可议。
- 纪律记档：W-T3b 超硬顶（140→171，+22%），超限部分为收尾最小路径——记任务书
  模板改进：预算检查点按证据项分段设，不设单总顶。环境已清理（实例/DSH/vite/
  代理全退，gui-config 还原备份，/tmp/wt3b-* 留作复跑）。
- 验收状态：W-T3 代码面 PASS 维持；合并验收裁定 BLOCKED 于硬项 #4，待修复卡
  （head.rs Origin/Referer 重写红绿 + 复跑 wt3b 注入链）翻转后出最终裁定。
- 资产：wt3b 注入驱动链可复用，为后续 e2e 的标准工具。

## 2026-09-11 用户裁定：W-T3c + W-T4 并行派发
- W-T3c（session-d2335ba2）：反代 Origin/Referer 重写修复小卡。文件域独占
  apps/gui/src-tauri/src/tunnel/** + specs/tunnel.md 重写节；红绿四段验收
  （红证据→单测绿→复跑 wt3b 链翻硬项 #4→门禁）；预算分段检查点，总顶 100。
- W-T4（session-875a1847）：流装配 double-write 清扫 + pump 半关真实 TCP 复核
  （W-T3 移交 MUST）+ llm-share 0.1.7 互通 + tunnel.md 装配 MUST 条款。
  文件域独占 crates/** + apps/cli/**；五段检查点，总顶 240；worktree 全量
  make check 绿后 push，禁自合并。
- 任务书模板改进（W-T3b 超限教训）：预算按任务段设检查点回报，段末超支预警，
  不设不可执行的单总顶。
- 验收裁定链：W-T3c 翻转硬项 #4 后，出 W-T3+W-T3b+W-T3c 合并最终裁定；
  W-T4 独立验收。两卡 push 后协调者独立复核（maker/checker 分离）再合并。
- [W-T3c 段①] 红证据完成：87a96701（head 2 断言 + proxy 端到端 2 断言修复前全红，
  失败原文与 wt3b 根因吻合；负例护栏先绿）。RED.md 已存档。预算 20/100。
- [W-T3c 段②] 单测绿：3635fb47（forward_conn 单一重写点覆盖 POST+WS 升级；无头不造头/
  null/localhost 不动；重写目标仅限票据 target 不扩信任面）。19 passed/0 failed，
  tunnel.md §2/§3.2/§3.3 + gui-contract §19.2 契约随码同步，clippy 全 workspace 过。
  预算 34/100。注：tunnel.md 与 W-T4 域有叠交，rebase 冲突按 brief 由 W-T4 侧消化。
- [W-T4 段1+2] @ e08ddb47：全仓 grep 兜底共清 3 处双写工厂（p2p-cli/borrow_dial.rs
  ——brief 路径 apps/cli 系陈旧、行号已漂移；itest tunnel_common/mod.rs:63；itest
  llm_share_common/mod.rs:108——brief 未列，grep 新抓），其余 2 处 StreamFactory 合法
  （SwarmFactory 本体/LoopbackHub），直接拨号方用 new_stream 自握手属正确口径。
  红绿 itest tunnel_assembly_wire.rs 3 探针 no-tolerance：红=真实夹具工厂被严格探针
  拒绝（首业务帧=协议 ID 第二帧原文）；绿=3/3 + tunnel_wave 7/7（容忍循环转纯防御）。
  llm-share 0.1.7 基线 12/12 清扫前后一致。预算 45/240。
- 协调者自记：任务书路径第二次漂移（apps/cli vs crates/p2p-cli），grep 兜底条款
  再次兜住——任务书模板固定要求「行号/路径仅供定位，以 grep 兜底为准」。
- [W-T4 段3] pump 半关真实 TCP 红绿定案 @ bb2b0595：pump 零修改、无缺陷。三腿证据：
  ①新增 pump_wire_tests.rs 泵级真实 TCP 探针（FIN 后 LATE 帧照收、字节计数精确、
  自然收口）②生产 yamux 全栈既有 itest h1_half_close_directions_independent 覆盖同
  场景 ③对照组：整流关闭读端收尽在途后 EOF 0 字节属 TCP 正确行为。根因=tokio
  duplex shutdown 只关写半 → duplex 测试复现不出；真实 TCP 测试误用整流关闭当写
  半关即得「FIN 后 0 字节」假象。W-T3 CONCERNS#3 MUST 闭环。预算 65/240。
- [W-T3c 段③] 硬项 #4 翻转 PASS @ 2f9002a7（EVIDENCE-wt3c.md）：WS 握手 101（3ms，
  Origin 重写过栅栏）、/api POST 全 200（wt3b 全 403 对照）、真实流式对话经隧道
  （kimi-k3「1+1等于2。」58tok/s）、审计 225 会话 ok156/busy26/io41/open2、
  bytesIn≈12.2MB。复跑环境注：/tmp/wt3b-dsh-home 持久化了 wt3b 反代地址，已换
  wt3c-dsh-home 隔离重起（教训：DSH_HOME 复跑前要查持久化状态，已补进复跑须知）。
- **新缺口（待裁决，未动手）**：crates/p2p-tunnel serve 栅栏 max_concurrent 默认 4，
  浏览器并行 burst 触发 busy→502、页面需刷新 1-2 次。候选修法=调默认上限/反代侧
  busy 重试，需契约对账（规范页 §5.2）+红绿。列下一波小卡候选。
- [W-T3c 预算] 260/100 超支记档，主因=e2e 环境坑（双代理抢端口/webview 节流/stale
  DSH 状态/provider 冷启动）。模板教训固化：e2e 段独立预算线。段④门禁已批准
  （~15 次），门禁不许跳。

## 2026-09-11 W-T3c DONE 接收与复核
- 分支 feat/wt3c-origin-rewrite @ 4b6f0713（含 rustfmt 收尾），rebase 于 main 头
  e7229f7a 已核实。提交链符合红绿纪律：红测试先行（87a96701）→修复（3635fb47）
  →证据（2f9002a7）→[dbg]清理（d596bb88）→DONE 报。
- 协调者 diff 复核：文件域全净（src-tauri tunnel 模块 + 契约 docs + orchestrator
  证据），crates/apps/cli/scripts 零触碰；核心修复与 brief 逐条一致（127.0.0.1
  字面量同信任判据、无头不造头、null/localhost 保留、重写仅限票据 target、安全
  边界注释）；测试覆盖 POST/WS 升级/负例三组。
- 独立全量 make check 复核中（worktree wt3c-verify）。绿则 ff-only 合并，
  随后出 W-T3+T3b+T3c 合并最终验收裁定。
- 预算终账：~270/100（超支主因 e2e 环境坑，已固化模板教训：e2e 段独立预算线）。
- 遗留裁决候选（下一波小卡）：serve 栅栏 max_concurrent=4 遇浏览器 burst 忙拒
  （调默认上限 vs 反代侧重试，需规范页 §5.2 契约对账）。
- [W-T3c 补记] 87fc5bdf 已 push（EVIDENCE-wt3c.md 补 busy 栅栏复现参数：峰值并发
  6-14 vs permit 4，冷加载确定性 502 资源 2-6 个，统计面 busy 26/225≈11.6%）。
  协调者复核门禁跑在 4b6f0713；合并前核 4b6f0713→87fc5bdf 增量（预计纯
  .orchestrator 文档，门禁不扫），增量非文档则重跑。

## 2026-09-11 ★ 隧道波最终验收裁定：W-T3+W-T3b+W-T3c PASS（已合并）
- W-T3c 独立复核：全量 make check EXIT=0（4b6f0713），增量 87fc5bdf 纯
  orchestrator 文档核验通过，验证覆盖分支尖。main==origin/main 基线核对后
  ff-only 合并，主树 @ 87fc5bdf 已推（pre-push 快速门禁 PASS）。
- **最终裁定 PASS**：DSH 隧道访侧全链落地——
  - 代码面：反代 Origin/Referer 重写红绿双向证据齐（红 4 断言→绿 19/19、
    lib 151/151、clippy -D warnings、cli-parity、pnpm 四件套全绿）；
  - e2e 面：双 GUI 实例真实拓扑，硬项 #4 翻转（WS 101、/api 全 200、真实
    流式对话 kimi-k3 经隧道、审计 225 会话 12.2MB）、回环证明、负例 401、
    serve 开关全 PASS；
  - 契约面：specs/tunnel.md §6 扩 Origin/Referer 重写、gui-contract §19.2 同步。
- 已知遗留（不阻塞裁定，进 backlog）：①serve 栅栏 max_concurrent=4 浏览器
  burst busy→502（裁决卡候选：调上限 vs 反侧重试，复现参数已量化在档）；
  ②remote-access 控制通道/page-registry 注册卡；③错误态与已开启卡并存展示
  可议；④控制通道 /screenshot 帧源滞后；⑤节点 swarm 通配监听记录备查。
- 在途：W-T4（段5+全量门禁收尾），push 后独立复核合并，其 brief 已指定
  tunnel.md 冲突由 W-T4 侧消化。

## 2026-09-11 W-T4 DONE 接收与复核
- 分支 feat/wt4-stream-assembly @ 7f91cd84（基线 e7229f7a），4 commits，9 文件
  349+/10- 全在授权域（crates/** + specs/tunnel.md），禁区零触碰，无迁移。
- 协调者 diff 复核：三处双写工厂全迁 open_raw_stream（含 grep 新抓的
  llm_share_common/mod.rs:108）；tunnel.md §2.1 装配 MUST 条款与 2026-09-11 裁决
  逐字一致（协议 ID 恰一帧/工厂裸流/禁包 new_stream/直拨自握手例外）；pump.rs
  仅测试模块挂载，"产品路径零修改"属实；契约注释互指 no-tolerance 回归。
- 自报门禁：全量 make check EXIT=0（第 2 轮 rendezvous_facade_link quic 握手
  10s 超时一次=满载 flake，与改动零关联，隔离复跑 3/3 绿，第 3 轮全量含它通过
  ——处置合规）。预算 100/240。
- 待办：rebase 到新 main（87fc5bdf，W-T3c 已合入）由 W-T4 执行并 force-with-lease
  推送；协调者独立全量 make check 后 ff-only 合并。

## 2026-09-11 ★ W-T4 验收合并：DSH 隧道计划全链收官
- 独立复核：wt4-verify 全量 make check EXIT=0（rebase 后分支尖 61ba8878）。
  main==origin/main 基线核对后 ff-only 合并，主树 @ 61ba8878 已推（pre-push
  快速门禁 PASS）。
- **W-T4 最终裁定 PASS**：装配卫生收官——3 处双写工厂全迁 open_raw_stream、
  严格 no-tolerance itest 3/3（红绿双证）、pump 半关疑点接线 artifact 定案
  （pump 零修改、真实 TCP 回归固化半关闭语义）、llm-share 0.1.7 互通 12/12
  清扫前后一致、tunnel.md §2.1 装配 MUST 条款入库。
- 本轮 DSH 隧道计划全链闭环：W-T1 契约冻结 → W-T2 被访侧（协议栈+serve 面）
  → W-T3 访侧（反代+GUI）→ W-T3b 真实拓扑 e2e 实证（抓出 Origin 缺口）
  → W-T3c 修复翻硬项 → W-T4 装配卫生。主干 @ 61ba8878 全量门禁绿。
- 可归档：W1、W-T1、W2、W-T2、W-T3、W-T3b、W-T3c、W-T4（全八卡）。
- 下一波候选（按优先级，均需用户确认后派）：①busy 栅栏并发裁决卡（数据在档）
  ②remote-access 控制通道注册卡 ③历史 backlog（rust-cache、ascii-var-guard
  覆盖面、acp-common fmt 盲点、DSH 内嵌 webview 里程碑、token 生命周期治理）。
- 本波过程沉淀：任务书路径两次漂移靠 grep 兜底条款救回；e2e 段独立预算线；
  分段检查点预算制；门禁必含清单（src-tauri 卡：clippy -D warnings；GUI 命令卡：
  cli-parity + ai-docs-sync）。
- [收尾确认] W-T4 自清完成（worktree/本地远端分支全删，其被超时打断的半删树
  自查为自身被杀链非外物污染，--force+后台 rm 补完）。协调者终检：worktree 仅剩
  主树 @ 61ba8878、feat/fix 分支零残留、main==origin/main。本轮计划审计链闭合。

## 2026-09-12 102 跨机验收事实核查（阻塞呈报）
- 102 = Debian 13 trixie **无头**：无 Xvfb、无 webkit2gtk、无 /Applications、DISPLAY 空
  → p2p GUI（tauri）在 102 不可跑（除非重环境改造，脆弱不推荐）。
- serve 面现状：仅 GUI 进程内（TunnelState），CLI/控制通道均无 face（W-T3b 缺口记录
  一致）→ 被访侧 responder 无法在 102 无头落地 = 跨机拓扑断链。
- 102 现有资产：node v24.18.0(nvm)+pnpm+cargo、~/dsh-lan-setup.md（实为 **114 机**
  的部署文档：DSH 0.1.0-rc.6 常驻 macOS 26.5.1 arm64 @192.168.0.114，nginx 8443
  TLS 反代→127.0.0.1:3080，LaunchAgent 自启——DSH-as-LAN-service 已是既成模式）。
- 裁定呈报用户（scope 决策）：甲=新增 headless serve 面卡解锁 102/114（§19.8 预留
  条款的对号落地，busy 栅栏裁决可顺路对账）；乙=102 上 Xvfb+webkit2gtk 跑 GUI
  （环境重，不推荐）；丙=本机双实例即终验，102 挂起 backlog。

## 2026-09-12 用户裁定：方案甲（headless serve 卡）→ W-T5 派发
- 用户选甲：新增 p2pctl tunnel serve 子命令解锁无头被访机，跨机验收照做。
- W-T5 已派发：session-4ad983de（类型 code-quality，五段分段预算总顶 200/段④60）。
  范围：crates/p2p-cli + apps/cli + crates/p2p-tunnel（busy 栅栏）+ 规范页/契约文档
  + 登记面（ai-guide/cli-parity 理由更新）；禁改 apps/gui/src-tauri。
  跨机拓扑：102 无头被访侧（repo clone + cargo build + npm 装 DSH）↔ mac 访侧 GUI
  （复用 wt3c 驱动链）。分支 feat/wt5-headless-serve，禁自合并。
- 待细化区（下一波候选，需用户确认后派）：remote-access 控制通道注册卡；
  busy 若 W-T5 对账未覆盖再议；历史 backlog（rust-cache、ascii-var-guard 覆盖面、
  acp-common fmt 盲点、DSH 内嵌 webview 里程碑、token 生命周期治理）。
- [W-T5 段①②] @ 38607896：p2pctl tunnel serve 落地（前台常驻/优雅收口逐条终态/
  stdout JSON 行 ready/stopped/stderr 结构化审计八字段，live 冒烟+单测 4 绿）。
  busy 栅栏对账红绿：RED 存档（默认 4，wt3c 峰值 14 并发 busy=10 ok=4）→默认 4→16
  转绿，语义探针（permit2+burst4→2 served+2 busy 逐条 Rejected(Busy)）绿，默认值
  单测钉死；GUI 零改动联动验证（tunnel.rs:43 default() 同源）。规范页 §5.2 裁决
  条款+§8 headless 行同步；三套件零回归。待段③登记面→段④102 跨机。
- [W-T5 段③④] @ 6204c4c6：登记面齐（AI-DOCS-OK 100 叶子/340 参数、CLI-PARITY-OK、
  豁免理由补 headless 对等面、gui-contract §19.3-8 落地）。**102 跨机全链绿**：
  102 clone+build p2pctl（bundle 传输）、DSH rc.6 @127.0.0.1:13080（systemd-run）、
  headless serve ready(peerId=67R7k4…/max 16)；mac 访侧 mDNS 跨机发现（非回环
  IPv6 传输地址）→全量加载（API 全 200）→真实流式对话 GLM-5.2（首 token 4.9s/
  100 tok/s）→双面审计对账（mac 178 会话 8.9MB ↔ 102 391 条八字段）→ss 证仅
  loopback→负例坏 Origin 403→SIGTERM 优雅收口（新流 rejected:shutdown 逐条终态、
  在途超时、显式非零退出=禁静默设计路径）。偏差如实登记：rc.6 无 WS 端点（数据面
  POST 流式+events 长连）、无 boot token（负例改非受信 Origin 403）。证据落 wt5/。
  现场已清。待段⑤门禁+push。

## 2026-09-12 W-T5 DONE 接收与复核
- 分支 feat/wt5-headless-serve @ 705c8f28（基于 main 头 61ba8878，无 rebase 需求），
  5 commits 提交链符合分段纪律（红绿先行→证据→fmt）。9 代码/文档文件全在授权域，
  GUI 路径零触碰。
- 协调者 diff 抽查：tunnel_common 裸流迁移完好（仅加 Clone）；busy 默认 4→16 带完整
  裁决注释链（证据指针/理由/GUI 同源联动/--max-concurrent 覆盖口）；TSV 豁免理由
  如实更新仍 exempt。
- 自报门禁为 12 目标拆分执行（前台 10 分钟硬顶），聚合目标未单跑——协调者独立
  全量 make check 复核中（worktree wt5-verify），同时补上聚合证据。
- 偏差登记可接受：rc.6 无 WS 端点/无 boot token，负例等价替换（坏 Origin 403），
  证据效力透明。

## 2026-09-12 ★ W-T5 验收合并：跨机验收补齐，隧道计划最终收官
- 独立复核：wt5-verify 全量 make check EXIT=0（含 W-T5 因前台硬顶拆分未跑的聚合
  目标，CLI 叶子 99→100=新 tunnel serve 命令已登记）。
- 过程波折（均无损害）：①W-T5 的 self-evolving 喂回提交 24936a13 直打主树 main
  （协议瑕疵，docs-only 已发布）→ 协调者 rebase 分支消化；②协调者首次 force-push
  推错 ref + 在 detached 树对旧 ref merge（即发现即纠正，main/远端零影响）；
  ③主树未跟踪证据文件与分支同名——备份后合并，diff 确认逐字节一致零丢失。
- 最终状态：main=origin/main @ 3a2a004a，pre-push 快速门禁 PASS；rebase 后分支尖
  复跑 fmt/ai-docs-sync/cli-parity 三绿。三 worktree + 分支全清（后台）。
- **W-T5 最终裁定 PASS**：headless serve 面（前台常驻/优雅收口逐条终态/stdout
  JSON 行）+ busy 栅栏裁决（默认 4→16，红绿双证）+ 102 跨机全链实证（mDNS 非回环
  发现、真实流式对话 GLM-5.2、双面审计对账、loopback 证明、负例 403、优雅收口
  实测）。cli-parity/ai-docs-sync/gui-contract §19.3-8/规范页 §5.2/§8 全同步。
- **★ 跨机验收（本机+102）就此补齐——DSH 隧道计划九卡全链闭环：**
  W-T1 契约 → W-T2 被访侧 → W-T3 访侧 → W-T3b e2e 实证 → W-T3c Origin 修复
  → W-T4 装配卫生 → W-T5 headless 跨机。可归档九卡：W1、W-T1、W2、W-T2、W-T3、
  W-T3b、W-T3c、W-T4、W-T5。
- 待细化区（需用户确认后派）：remote-access 控制通道注册卡；历史 backlog
  （rust-cache、ascii-var-guard 覆盖面、acp-common fmt 盲点、DSH 内嵌 webview
  里程碑、token 生命周期治理、busy 栅栏进 GUI 设置项）。
- [收尾补记] W-T5 针对协议瑕疵自纠偏：分支 docs/skill-wt5-redline 补技能红线一行
  （经验喂回须走分支流程禁直打 main，a9ea63e1），协调者 ff 合并入 main @ a9ea63e1
  并推送（pre-push 快速门禁 PASS）。终态：worktree 仅主树、feat/fix/docs 分支零
  残留、main==origin/main。过程债务清零。
