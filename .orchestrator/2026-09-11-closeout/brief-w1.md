# 任务 W1：定位并修复 affected-fast 自测在 make check 编排下的必红（CI 卡口）

## 目标
让 main 主干 `make check` 恢复全绿：定位 `scripts/check/tests/affected-fast.sh` 在 `make check` 编排下必红的根因并修复，
并以"同命令同环境、改前必红 / 改后必绿"的双向证据交付。这是当前唯一挡住所有合并与 CI 的卡口。

## 背景（指针，不是内容）
- 本任务在计划中的位置：收口轮 W1（最高优先级）。完成后主干门禁恢复，W2（GAP 规范回写）才能合入。
- 必读输入：
  - `AGENTS.md` —— 硬性工作规范（worktree 协议、提交纪律、失败路径可观测、行数红线）
  - `scripts/check/tests/affected-fast.sh` —— 报告问题的自测脚本本身（186 行，断言函数 `assert_field` 在 27-34 行）
  - `scripts/check/affected.sh` / `scripts/check/fast.sh` —— 被自测的编排脚本（`CHECK_ROOT` 夹具钩子是关键）
  - `Makefile` —— `gate-tests` 目标逐条串行调用 8 个自测脚本，`affected-fast.sh` 是最后一条
  - `.agents/skills/self-evolving/references/known-issues.md`（第 543 行条目）—— 同族历史事故：bash 脚本 `set -u` 下 unbound variable 噪音
- 已核实的环境事实（沿用，不要重复劳动）：
  - 本机存在两个 bash：`/bin/bash` 3.2.57（macOS 自带）与 `/opt/homebrew/bin/bash` 5.3.9。
  - `make` 的隐式 recipe 用裸 `bash script.sh`，走 PATH 里的那个；本仓多数脚本靠 `export PATH` 把 `/opt/homebrew/bin` 放前面，
    但 Makefile 的 PATH 与交互 shell 不同（见下方"复现命令"）。两个 bash 都试过，单独跑脚本两者都绿。
  - `gh` CLI 不在本机 PATH；GitHub Actions job 日志需 admin 权限，本仓 token 拉取返回 403。**不要尝试去拉 CI 日志**，
    以本地复现为唯一证据来源。

## 已知事实与复现证据（协调者已跑，逐字保留）
1. GitHub `ci` 工作流在 main 上连续三个 run 全 failure，失败步只有「全量门禁」（其余步骤 success）：
   run 34472747421(4212ea8) / 34475371238(8497b2f) / 34477498790(e91119e)。
2. 本地等价复现（**改前必红证据，请先自己复跑确认**）：
   ```
   cd /Users/imeepos/ext512/p2p
   export PATH="$HOME/.cargo/bin:$PATH"
   CI=true make check > /tmp/w1_before.log 2>&1
   echo "MAKE_CHECK_EXIT=$?"
   ```
   实际观测：`MAKE_CHECK_EXIT=2`；`/tmp/w1_before.log` 尾部为
   ```
   bash scripts/check/tests/affected-fast.sh
     ok   干净树 FULL_ALL=0
     ...（前 14 条 ok）...
   scripts/check/tests/affected-fast.sh: line 32: name<乱码>: unbound variable
   make: *** [gate-tests] Error 1
   ```
   注意：**前 14 条断言全 ok 且不打印 FAIL 行**，错误直接出现在第 15 条用例（`fast.sh 端到端退出 0`）之前的时段。
3. 单独跑同一脚本稳定绿（4 次，两种 bash 各若干）：`bash scripts/check/tests/affected-fast.sh` → `affected-fast: pass=20 fail=0`，exit 0；
   `CI=true make gate-tests` 单独跑也绿。→ **只在 make check 全量编排下复现**，疑似编排/负载/环境相关，不是随机 flake。
4. `line 32: name<乱码>` 中的乱码是 UTF-8 多字节序列（`（` 的字节），说明第 32 行的 `$name（期望行: $want；实际: ...）`
   在某种状态下被 shell 当成变量名解析——根因大概率与 `assert_field` 的调用/展开路径有关，而不是简单"某个断言失败"。
   **不要停在猜因**：必须能说清"哪一步让 `${name...}` 成为 unbound"，并给出可重复的复现。

## 验收标准（全部满足才可报 DONE）
- [ ] 根因写清楚：贴出最小可重复复现命令 + 实际输出，指出具体文件行与机制（不是"看起来像"的推测）。
- [ ] **改前必红**：修复前，用第 2 条命令复现 `MAKE_CHECK_EXIT=2`（贴命令与退出码）。
- [ ] **改后必绿**：修复后同一条命令 `MAKE_CHECK_EXIT=0`，且全量 `make check` 的每个子门禁 PASS 行齐全（贴证）。
- [ ] **红绿双向锁死**：把根因固化为自测用例或机械守卫，使该缺陷再次引入时 `make check` 立即红（贴"注入缺陷→红→还原→绿"证据）。
- [ ] `CI=true make check` 在主树（或你的 worktree，两者择一但必须说明是哪个树、跑了几轮）至少 **连续 2 轮** exit 0。
- [ ] worktree 内 `make check` 全绿（这是合并前置条件）；分支已 push 到 origin。
- [ ] 每步命令都用 `cmd > log 2>&1; echo EXIT=$?` 形态取真实退出码，**禁止用管道尾命令的 `$?` 冒充**。

## 边界（明确不做）
- **禁止**为了让门禁变绿而放宽/删除断言、加 `|| true`、跳过或 `set +u` 掉 `set -u`——那等于拆掉防假绿机制；
  若你的结论确实是"脚本逻辑本身错了"，要给出为什么原断言无效的论证，并保证等价或更强的守卫仍在。
- 不改 `scripts/check/**` 与 `Makefile` 之外的文件；**不碰** `docs/protocol/**`、`crates/**`、`apps/**`（W2 与其他轨在途）。
- 不改 `.agents/skills/**`（技能文件由协调者维护）。
- 不动 CI workflow 定义（`.github/workflows/**`）——除非根因确实在其中，且此时先报 BLOCKED 说明，不要自行改。
- 不引入新依赖、不新增外部工具（无 docker、无 gh）。
- 不清理/不删除他人 worktree；`.orchestrator/**` 只读。

## 接口契约
- Consumes：main @ `4df3eb5`（协调者已 push origin，基线即此提交）；本任务书列出的复现命令与观测。
- Produces：主干 `make check` 全绿的可复现状态 + 一条针对该根因的红绿回归守卫（在 `scripts/check/tests/**` 内）；
  后续 W2 与所有波次卡都消费"主干门禁绿"这一事实。

## 预算与停止条件
- 预算：约 4 轮修复 / 累计工具调用上限约 40 次（跑 `make check` 前先用最小复现收敛，别每轮都跑全量）。
- 立即停止并报 **BLOCKED**：根因确定为 Linux/macOS 平台差异或 CI runner 特有行为、本地无法复现；
  或修复必然需要改 `crates/**`、`apps/**` 生产代码；或需要 CI 日志权限。
- 立即停止并报 **NEEDS_CONTEXT**：本任务书的复现命令在基线 `4df3eb5` 上跑不出红（此时请贴出你的实际退出码与日志尾部，
  协调者会重新标定事实）。

## 工作区与汇报
- 分支 `fix/w1-gate-affected-fast`，worktree `.worktrees/w1-gate`（`git worktree add .worktrees/w1-gate -b fix/w1-gate-affected-fast 4df3eb5`）。
- 遵循 `AGENTS.md` 收尾纪律：worktree 内 `git rebase main` 反向同步 → 全量门禁 → push origin → 报 DONE，
  **不要自行合并 main**（合并由协调者执行）。只 add 自己范围的文件，禁用 `git add -A` / `commit -a`。
- 提交纪律：`type(scope): subject`，正文写机理（why）。一次提交一件事；回归守卫与其修复可同提交（配套）。
- 汇报经 `session_link_send_parent` 回报派发你的主会话（自动解析上级）。
  派发者显式 id：`session-3373f897-f9c0-4a32-9af2-b9562eecc311`（`send_parent` 解析异常或发送失败时改用它定向投递，不重发试错）。
- 汇报状态只用：DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT。
- DONE 汇报必须附：变更文件清单、每条验收标准的核验证据（命令 + 原始退出码 + 关键输出行）、顾虑，以及结构化复盘：
  - 最耗时的是什么？技能/本任务书有没有提前警告？重来一次会怎么做？

## 早落盘纪律（防宿主静默杀死）
- 开工先建 worktree 并落一个 `PROGRESS.md`（放 worktree 根，勿提交），每完成一步追加一行：时间 / 当前状态 / 下一步。
- 任何超过 5 分钟的命令放后台并留日志文件路径；不要在没有盘上痕迹的状态下长跑。

## 附录：复现条件更正（2026-09-11，协调者补，权威）
- 原任务书第 2 条"改前必红"命令**不完整**：`CI=true make check`（不设 locale）实测 **EXIT=0 全绿**，复现不了。
- 真正触发条件 = **多字节 locale**（bash 3.2 与 5.3.9 均复现）：
  `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 bash scripts/check/tests/affected-fast.sh` → exit 1，
  尾部 `line 32: name<乱码>: unbound variable`。
- 机理：bash 在多字节 locale 下把 `$var` 后紧跟的非 ASCII 字符并入变量名，`set -u` 击杀脚本；
  C locale 不触发（故"本机绿、CI 红"）。
- 因此"改前必红/改后必绿"两条命令必须逐字带 locale 变量。
