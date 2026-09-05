
# 无论任何任务 skill: `self-evolving` 总是需要遵守的
> 技能文件地址：`.agents/skills/self-evolving/SKILL.md`
> 使用worktree避免冲突，完成后合并到主分支，合并成功后清理老分支确保代码安全，不用用户同意
> 写完的东西要立刻存档（commit）
> 禁止直接在主分支上修改任何代码
> 102服务器信息： imeepos@192.168.0.102
> dispatch 超时 ≠ 任务死亡 接管前先查产物再动手 避免双写

- **短命分支**：任务完成当天即合并，不过夜；冲突一律在 feature 侧消化，进 main 的合并保持干净。
- **合并前反向同步**：worktree 内先 `git merge main`（本地私有分支可 rebase），解冲突跑门禁，再回主树合并。
- **中央登记文件 append-only**：menu.def.ts / App.tsx / i18n types+locale / fields.md 的注册类改动压成独立小提交，不埋进大 feature 提交。
- **收尾四步（硬性，防代码丢失）**：① `git push origin <分支>`（2026-09-05 用户拍板：本项目不用 gitea，远端一律 origin）→ ② 主树 `git merge --ff-only <分支>` → ③ `git worktree remove` → ④ `git branch -d` + `git push origin --delete`。**执行前先核对 cwd 在主树（`pwd` + `git branch --show-current`）**：在 feature worktree 内执行②是 no-op 假成功（2026-08-28 实例），还会把本地 main 指针误推。
- **ff-merge 失败 ≠ commit 丢失**（commit 安全在分支 ref 上）：失败时严禁删 worktree，唯一动作是回 worktree `git rebase main` 后重试②。
- 误闯并行会话的 worktree 并编辑其未提交文件是事故（2026-08-22 用户点名）；发现半成品先 `git worktree list` 判断归属。

## 迁移编号规则（防并行撞号，2026-08-22 固化）
worktree 只隔离文件，不隔离全局共享的流水资源（迁移号/路由/权限码/契约章节），
各自"最大号+1"必撞（当日 000102/000103 连环撞号两次）。规则：
- 新增迁移前必查两处：`ls migrations | tail`（本树）+ `git for-each-ref refs/heads` 逐分支
  `git ls-tree <分支> -- migrations/`（未合并分支占号）。
- `make check` 的 check-contract-sync D 项机械拦截：同树撞号 + 跨未合并分支撞号，红了必须让号，
  不许进 baseline 豁免（历史撞号 000044/000095 为存量例外，已登记）。
- 让号规则：已合并进 main 者优先，后来者让号；未进任何库的迁移直接改名即可；
  已进库的改名必须同步 `UPDATE schema_migrations`（见 edd0666 先例）。
- **占号顺序（2026-09-01 双会话撞号实证）**：并行轮先 `git fetch origin && git merge main`
  反向同步**再**定迁移号，比"各自取最大号+1"可靠——后查号只能事后让号，先同步可直接避开。

## 提交纪律（revert 可行是硬约束）
- message 一律 `type(scope): subject`（feat/fix/refactor/docs/test/chore/style），正文写机理（why），不只写标题。
- 一次提交 = 一个可独立陈述的变更；feat 带测试、fix 带回归、契约变更带 fields.md 同步，配套随主变更同提交。
- 巨石提交仅限纯结构迁移（零行为变更）；行为变更禁止一锅端（不许 feat+fix+重构混装、不许多个不相关能力塞一个提交）。
- 判断标准：这个提交能否被单独 revert 而不伤邻居？不能就拆。

## 数据核查与改动自查红线（2026-08-29，全文见 docs/notes/adopted/2026-08-29-audit-closeout-rulings.md §7）
- **先查库、再接口复核**：任何"孤儿/不一致"结论必须先 SQL 直查权威表，再以接口复核读路径（"孤儿支付"先例系读路径假象）。
- **改动文件前自查余量**：目标文件行数（红线 300）+ 磁盘剩余空间；make check C 项机械兜底行数。
- **并行会话基线**：合并前核对本地 main 与远端一致再 ff-only；测试运行与工作区改写严禁对同一 worktree 并发。
- 验收/E2E 造数不过夜：mainchain-acceptance.sh 收尾自动 acc_ 清理 + 孤儿巡检门禁；102 每日 cron 见 docs/ops/patrol-cron.md。

## 已知环境事实
brew 和 graphviz 都在 /opt/homebrew/bin

编码要求：
- 单个文件不要超过300行，推荐200行以内
- 单个函数/方法不要超过60行，推荐40行以内
- 注释不要废话/套话，函数名/方法名就是最好的注释
- 能复用就不要早轮子
- 不要使用emoji图标
- 读文件请使用Read工具
- 编辑文件前请务必先读取文件
- 失败路径必须留有可观测信号（告警日志或显式状态），禁止静默吞错

## 密钥信息
存放在: .env
cargo: $HOME/.cargo/bin