# TA 任务书：authz 角色管理后端（core op + tauri 命令 + 契约/tsv 登记）

> 类型: backend(rust) | 分支: feat/wra-backend | worktree: /Users/imeepos/ext512/p2p/.worktrees/wra-backend
> 派发者显式 id: session-31ed5fb1-8ca8-4500-9ee6-1fa8a9c1c81d（send_parent 解析异常时定向回投）

## 目标

为 GUI「指定角色权限」补后端能力：p2p-authz 增 update_role op；src-tauri 增
authz_permissions_list / authz_role_create / authz_role_update / authz_role_delete
四命令；cli-parity.tsv 登记四行；gui-contract.md §18 增补。

## 契约（逐字，禁改名；与 TB 卡共享，任何偏离=BLOCKED）

只做 plan.md §2 所钉契约。工作树内读 `.orchestrator/2026-09-12-authz-role-gui/plan.md`
（主树路径同文件，worktree 建立后同相对路径可读）。

关键实现点（与既有代码对齐）:
- crates/p2p-authz/src/ops.rs: `Authz::update_role`——仅自定义角色可改（builtin 显式
  AuthzError 拒；未登记 RoleNotFound）；permissions 走既有 parse_perms；role_id 不可变；
  save_roles(custom_roles) 落盘。ops_tests.rs 补测试: 改自定义成功、内建拒、未登记拒、
  表外 key UnknownPermission 拒、落盘重读一致。
- apps/gui/src-tauri/src/authz.rs: 四命令复用 p2p_cli::authz 与 p2p_authz 既有函数；
  报告类型 camelCase 透出（沿 AuthzBindingsReport 先例）。authz_role_delete 在
  state.config_get().authz_default_role == role_id 时返回可读中文 Err（防悬空默认角色）。
  lib.rs invoke_handler 注册四命令。
- 报告形状（serde rename_all camelCase）:
  AuthzPermissionsReport { permissions: Vec<String> }；
  AuthzRoleMutationReport { role: RoleListReport 的角色视图同形状 }（注意: role_list 返回的
  RoleListReport 内部角色形状即 AuthzRoleView 契约形状，保持同源）；
  AuthzRoleDeleteReport { roleId: String }。

## 输入（读这些，不猜）

- crates/p2p-authz/src/{ops.rs,ops_tests.rs,errors.rs,permissions.rs,role.rs}
- apps/cli/src/authz/{mod.rs,role.rs}（role_list/create/delete 的 CLI 逻辑层签名）
- apps/gui/src-tauri/src/{authz.rs,lib.rs,state.rs} 与 apps/gui/src/lib/ipc-types.ts §18 块（只读，前端形状对齐参照）
- scripts/check/cli-parity.tsv（authz 段 76-82 行先例）+ scripts/check/cli-parity.sh
- docs/design/gui-contract.md §18、docs/design/authz-role-design.md §4/§5/§10

## 验收标准（可机械判定，逐条给证据）

1. `cargo test -p p2p-authz` 退出码 0，含 update_role 新增测试（上列五态）。
2. `bash scripts/check/gui-tauri.sh` 退出码 0（本机 macOS 真跑 cargo test/clippy src-tauri）。
3. `bash scripts/check/cli-parity.sh` 退出码 0（tsv 四行登记后）。
4. `bash scripts/check/clippy.sh` 与 `bash scripts/check/fmt.sh` 退出码 0。
5. `bash scripts/check/line-limit.sh` 0（单文件 ≤300 行；authz.rs 现约 150 行，加四命令若超限
   则拆 authz_role_admin.rs 新模块，注册进 lib.rs）。
6. gui-contract.md §18 增补「§18.5 角色管理命令面」: 四命令表 + 校验语义 + 默认角色删除闸；
   措辞对齐 §18 既有行文；ipc-types.ts §18 注释块内容与其逐字一致（ipc-types.ts 本身归 TB 改，你只读）。
7. panic 卫生: 新代码非测试路径零 unwrap/expect/panic（bash scripts/check/panic-hygiene.sh 0）。

## 边界（明确不做）

- 禁触 apps/gui/src/ 任何文件（TB 卡所有权）；禁触 i18n；禁触 menu.def.ts/App.tsx。
- 不改 CLI 子命令集（role update 的 CLI 对等是后续轮，tsv 里 exempt 即可）。
- 不改权限闭集表（permissions.rs 的 REGISTRY 九 key 不动）、不改内建角色。
- 不做 yrs 化、不做跨面踢会话（设计 §14 开放问题，不属本波）。

## 提交纪律

- feat(p2p-authz): update_role op + 测试 —— 独立提交。
- feat(gui-tauri 或对应 scope): 四命令 + 注册 + tsv 登记 + 契约文档增补 —— tsv/契约
  登记随主变更同提交（契约变更带文档同步）；若你判断登记应独立小提交（AGENTS.md
  中央登记文件规则），允许拆两个提交，但禁止混进不相关变更。
- message: `type(scope): subject`，正文写机理。

## 预算与停止条件

- 修复轮 ≤3；预估 ≤10 次工具调用不现实则提前回报拆分建议。
- 立即 BLOCKED: 契约与既有代码冲突（如 RoleListReport 形状对不上）、门禁脚本红且
  无法归因到本次改动。回报用 session_link_send_parent，状态枚举
  DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT，DONE 附每条验收的命令+退出码。
- 早落盘小步写: 先提交 core op，再做命令层（派发后主控会验 worktree 落盘迹象）。
