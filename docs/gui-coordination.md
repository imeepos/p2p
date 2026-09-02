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

## W1 骨架波（并行，进行中）

| 单 | 会话 | 分支 | 范围（文件所有权） | 验收（机械命令） | 状态 |
|---|---|---|---|---|---|
| G-A tauri 桥接骨架 | GUI-A（新建会话） | feat/gui-bridge | `apps/gui/src-tauri/**` | `cargo clippy -- -D warnings` + `cargo test`（src-tauri 内）全绿；契约类型 serde roundtrip 单测在列 | 进行中 |
| G-B 前端骨架 | GUI-B（新建会话） | feat/gui-shell | `apps/gui/**` 除 src-tauri | `pnpm -C apps/gui build` 零错误；骨架含路由/侧栏/顶栏/状态栏/主题/双语/AsyncButton/toast/AlertDialog/ipc+mock+store；menu.def.ts 六项注册 | 进行中 |

## W2 视图波（并行，待 W1 合并后派单）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-C 监控视图 | feat/gui-views-monitor | 仪表盘/节点/事件三视图 + 各自 i18n/menu 不新增 | `pnpm -C apps/gui build` + mock 模式可演示三视图数据流 | 待派 |
| G-D 配置视图 | feat/gui-views-config | 设置/发现/中继三视图 + 表单校验 + 危险操作 AlertDialog | `pnpm -C apps/gui build` + mock 模式保存/恢复往返 | 待派 |

## W3 集成打磨波（待 W2 合并后派单）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-E 集成联调 | feat/gui-integration | 真实 IPC 贯通、双节点冒烟脚本 scripts/gui-smoke.sh、缺陷修复 | 脚本跑通双实例 mDNS 发现 + ping；build 零错误 | 待派 |
| G-F 体验打磨 | feat/gui-polish | a11y/键盘/空态/错误态/加载骨架/i18n 完整性/主题一致性 | build 零错误 + 打磨清单逐项勾选 | 待派 |

## W4 收尾波（待 W3 合并后派单）

| 单 | 分支 | 范围 | 验收 | 状态 |
|---|---|---|---|---|
| G-G 打包回归 | feat/gui-package | `pnpm tauri build`、README（GUI 章节）、最终回归 | 产物可启动 + make check 仍全绿 + 回归清单 | 待派 |

## 变更记录

- 2026-09-02 23:40 协调会话创建本表；规划/契约冻结；W1 双单派发（G-A/G-B）。
