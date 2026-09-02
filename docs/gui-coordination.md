# GUI 并行开发协调表（p2p-console）

维护人：GUI 协调会话（本会话）。各 GUI 开发会话只读；根协议协调表 [coordination.md](coordination.md) 不受影响。
规划与契约：[design/gui-plan.md](design/gui-plan.md) + [design/gui-contract.md](design/gui-contract.md)。

## 通用规则（各会话必读）

1. 分支/worktree：`git worktree add .worktrees/<名字> -b <分支>`；只改自己范围文件；严禁 `git add -A`。
2. 完成即报：分支名 + 提交列表 + 验收命令输出摘要，回报 GUI 协调会话；**不自行合并 main**，由协调者统一合并。
3. cargo 在 `$HOME/.cargo/bin`；前端包管理一律 pnpm；远端为 origin。
4. 红线：不改 crates/**；不改 docs/coordination.md；文件 ≤300 行、函数 ≤60 行；无 emoji；失败路径必须可观测。
5. 契约（gui-contract.md）冻结：缺口只能报协调会话加法修订，禁止私自改名。
6. 注册类文件（menu.def.ts、i18n types+locale）的变更压成独立小提交。

## W1 骨架波（已完成）

| 单 | 会话 | 分支 | 范围（文件所有权） | 验收（机械命令） | 状态 |
|---|---|---|---|---|---|
| G-A tauri 桥接骨架 | GUI-A（session-0b527964） | feat/gui-bridge | `apps/gui/src-tauri/**` | `cargo clippy -- -D warnings` + `cargo test`（src-tauri 内）全绿；契约类型 serde roundtrip 单测在列 | ✓ 已合并（97957e0，00:33；clippy 零告警+29 用例，gui-gate 复验 PASS） |
| G-B 前端骨架 | GUI-B（session-d34300e8） | feat/gui-shell | `apps/gui/**` 除 src-tauri | `pnpm -C apps/gui build` 零错误；骨架含路由/侧栏/顶栏/状态栏/主题/双语/AsyncButton/toast/AlertDialog/ipc+mock+store；menu.def.ts 六项注册 | ✓ 已合并（2b1726d，00:33；i18n 84=84、build 432KB js，gui-gate 复验 PASS） |

## W2 视图波（已完成）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-C 监控视图 | feat/gui-views-monitor | 仪表盘/节点/事件三视图 + 各自 i18n/menu 不新增 | `pnpm -C apps/gui build` + mock 模式可演示三视图数据流 | ✓ 已合并（a823e6a，02:55；二次 rebase 解 locale/store 冲突，复验全绿） |
| G-D 配置视图 | feat/gui-views-config | 设置/发现/中继三视图 + 表单校验 + 危险操作 AlertDialog | `pnpm -C apps/gui build` + mock 模式保存/恢复往返 | ✓ 已合并（ed83c7f+cdec78e，02:52；build/eslint/vitest 15/i18n 200=200） |

## W3 集成打磨波（进行中，G-E 的 Rust 侧已由 G-A2 提前交付）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-E 集成联调 | feat/gui-integration | 真实 IPC 贯通、双节点冒烟脚本 scripts/gui-smoke.sh、缺陷修复、事件 tsMs 字段补齐 | 脚本跑通双实例 mDNS 发现 + ping；build 零错误 | 待派（含 tsMs 跟进单） |
| G-F 体验打磨 | feat/gui-polish | a11y/键盘/空态/错误态/加载骨架/i18n 完整性/主题一致性 | build 零错误 + 打磨清单逐项勾选 | 进行中 02:54 派 GUI-D（两阶段：配置侧+壳先行，monitor 合并后补量） |

## 插队跟进单（W2 进行中即派，保持流水线满载）

| 单 | 会话 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|---|
| G-A2 桥接增强 | GUI-A（session-0b527964） | feat/gui-bridge-tsms | src-tauri/** + scripts/gui-smoke.sh：事件 tsMs、双节点无头冒烟测试 | clippy+test 全绿含冒烟测试；gui-smoke.sh PASS | 进行中 01:50 派单 |
| G-B2 前端基建 | GUI-B（session-d34300e8） | feat/gui-shell-infra | ESLint/Vitest/i18n-diff 脚本/Cmd+K 面板与快捷键基建（不碰 views/**） | lint+test+build+i18n-diff 全过 | 进行中 01:50 派单 |

## W4 收尾波（待 W3 合并后派单）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-G 打包回归 | feat/gui-package | `pnpm tauri build`、README（GUI 章节）、最终回归 | 产物可启动 + make check 仍全绿 + 回归清单 | 待派 |

## 变更记录

- 2026-09-03 02:57 W2 全部合并（G-C a823e6a / G-D ed83c7f+cdec78e）；G-B2 合并（433a05b）；G-F 派 GUI-D 两阶段；G-A3 派 GUI-A；裁决：rendezvous 手动注册/查询为 v1 非目标（底座 pub(crate) 未暴露，CLI 已覆盖）。
- 2026-09-03 01:51 应协调指令保持流水线满载：向闲置的 GUI-A/GUI-B 派插队跟进单 G-A2（tsMs+双节点冒烟基建）与 G-B2（lint/test/快捷键/i18n-diff 基建），范围与 W2 视图波零冲突。
- 2026-09-03 00:35 W1 双单验收合并：A（11 提交，clippy 零告警+29 测试）+ B（12 提交，i18n 84=84），gui-gate 全绿 + make check 回归 PASS；契约加法修订 tsMs 可选字段与 §3 语法示例修正；B 的 pnpm allowBuilds esbuild 放行属工具链必需（e5 会话同样受益）；W2 双单派发（G-C/G-D）。
- 2026-09-02 23:40 协调会话创建本表；规划/契约冻结；W1 双单派发（G-A/G-B）。
- 2026-09-02 23:43 契约澄清修订：peer_dial target 语法 "<peer_id>@<addr>"、identity_reset 返回 NodeStatus（commit 4bc398f）。
- 2026-09-02 23:44 回填 W1 会话 ID；会话经 session_link 新建（专属会话，不复用历史会话）。
