# 任务 W-T1：/p2p-base/tunnel/1 线格式与准入契约（规范页 + 三处登记 + GUI 契约章节）

类型：architecture（产出是约束后续两卡实现的契约，不是运行代码）

## 目标
把下面【冻结契约】落成可判定的规范页与登记行，使 W-T2（被访侧实现）与 W-T3（访侧实现）无需再问你任何格式问题即可开工。

## 必读输入（指针）
- `docs/protocol/spec-charter.md` —— 规范章程：§8 章节骨架与状态口径、§9 登记纪律
- `docs/protocol/specs/repair-mcp.md` —— **结构最接近的先例**：协议 ID → 票据帧 → 双向字节分块；照它的章节骨架写
- `docs/protocol/specs/relay.md` / `circuit.md` —— 底座协议页的状态行与出处行体例
- `docs/protocol/registry.toml` —— 16 条现有注册项，新条目格式照抄
- `docs/protocol/wire-protocol.md` §3.2 —— 协议 ID 登记表
- `docs/design/remote-support-plan.md`（§34-36 附近）—— tunnel 类 ID 的既有裁决口径
- `docs/design/gui-contract.md` —— 契约文件体例（当前到 §18；本卡追加新章节）
- `scripts/check/protocol-registry.sh` —— 四向机械门禁（改完必须 PASS）
- `.orchestrator/2026-09-11-dsh-tunnel/proposal.md`（在第 18 行所在路径不存在于 worktree，**内容已在任务书与冻结契约中，不需要它**）

## 交付清单
1. **新规范页** `docs/protocol/specs/tunnel.md`（≤300 行）：含章程要求的完整章节骨架（概览/线格式/语义/约束/兼容与版本/测试向量/实现状态与出处/漂移登记），
   内容与【冻结契约】逐字一致；状态行按既有体例（如 `状态：planned；自 2026-09-11；归属 crates/p2p-tunnel`——实现状态以 W-T2/W-T3 交付后为准）。
2. **`docs/protocol/registry.toml` 追加一条** `/p2p-base/tunnel/1`：`impl` 字段按真实状态填（本卡落地时实现尚未合并 → 用 `planned`；
   W-T2 合并时会改 `implemented`，**W-T1 不得为凑门禁把它写成 implemented**）。
3. **`docs/protocol/wire-protocol.md` §3.2 表补一行** `/p2p-base/tunnel/1`。
4. **`docs/design/gui-contract.md` 追加新章节**（加法，版本号顺延到 §19）：登记两条 Tauri 命令 `tunnel_open_dsh` / `tunnel_status`
   与事件 `tunnel_status`，字段按冻结契约 §7 写全（含 `TunnelOpenResult` / `TunnelStatus` 形状），并注明"准入：仅回环监听 + 显式目标白名单 + 按次开启"。
5. **登记类改动压成独立小提交**（append-only 纪律）：规范页一个提交、registry/wire-protocol 登记一个提交、gui-contract 章节一个提交。

## 验收标准（全部满足才可报 DONE）
- [ ] `docs/protocol/specs/tunnel.md` 在位且 ≤300 行（贴 `wc -l`），章节骨架与 `specs/repair-mcp.md` 同级可比。
- [ ] 契约六项逐条在页面上可判定：帧序与超时、票据字段表（含类型与取值域）、应答与错误码闭集、数据面半关闭语义、
      准入与审计字段、访侧 Host 重写与流式转发约束；每项写成 MUST/MUST NOT 语句，禁"建议/尽量"。
- [ ] `bash scripts/check/protocol-registry.sh` → **PASS**（贴命令与退出码）；若因 `planned` 状态被判红，按脚本要求补齐双向核对所需的最小产物并说明。
- [ ] `bash scripts/check/ai-docs-sync.sh` → 退出码 0；`bash scripts/check/line-limit.sh` → PASS。
- [ ] `docs/protocol/registry.toml` 新条目与 `wire-protocol.md` §3.2 行**字面一致**（贴 grep 两处输出）。
- [ ] `docs/design/gui-contract.md` 新章节登记了两条命令 + 一个事件 + 两个结果类型字段表（贴 grep 输出）。
- [ ] worktree 内 `git rebase main` 后 push `origin docs/wt1-tunnel-spec`；**不要自行合并 main**。

## 边界（明确不做）
- **不写任何 Rust 代码**，不新建 `crates/p2p-tunnel`，不改 `crates/**`、`apps/**`、`scripts/**`、`Makefile`。
- 不改 `docs/protocol/specs/tunnel.md` 之外的既有规范页语义；不改 `registry.toml` 既有 16 条。
- 不改 `examples/mininode-python/**`、`.agents/skills/**`、`.orchestrator/**`。
- 不把 `impl` 写成 `implemented` 来"提前转绿"。
- 与其他卡的文件域边界：W2（在途）占 `docs/protocol/specs/rendezvous.md`、`ping.md`、`wire-format.md`、`quickstart.md`、`vectors/**` ——
  **本卡只碰 `wire-protocol.md` §3.2 表那一段与 registry 追加**，若与 W2 在途改动冲突，等你 rebase 时在 feature 侧消化；撞上就报 DONE_WITH_CONCERNS 说明。

## 预算与停止条件
- 预算：约 2 轮修复 / ≤25 次工具调用。
- 报 **BLOCKED**：冻结契约与 `spec-charter` 或既有规范体例冲突且无法调和；或 registry 门禁要求的产物本卡无权产出。
- 报 **NEEDS_CONTEXT**：契约某条无法写成可判定语句（例如错误码闭集与门禁要求冲突）。

## 工作区与汇报
- `git worktree add .worktrees/wt1-spec -b docs/wt1-tunnel-spec 4df3eb5`，在 worktree 内工作；先落 `PROGRESS.md`（勿提交）。
- 只 add 自己范围文件，禁 `git add -A`；message 用 `type(scope): subject` 并写机理。
- 汇报用 `session_link_send_parent`；异常时 `session_link_send` 到 `session-3373f897-f9c0-4a32-9af2-b9562eecc311`。
- 状态只用 DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT；DONE 附：文件清单（逐文件 `wc -l`）、每条验收的命令+退出码+关键输出、
  契约六项落点表（项目 | 规范页章节号 | 语句摘要）、顾虑、结构化复盘（最耗时 / 任务书是否提前警告 / 重来怎么做）。
