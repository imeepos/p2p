# 波次计划：GUI 角色权限管理（authz 角色创建/编辑/删除 GUI 化）

> 日期: 2026-09-12 | 主控: session-31ed5fb1-8ca8-4500-9ee6-1fa8a9c1c81d | 计划版本: v1
> 来源: 用户直接指令「目前的 GUI 中无法指定角色权限需要优化请修复」

## 1. 背景与事实源

- 设计: docs/design/authz-role-design.md（§4 权限闭集九 key、§5 内建四角色、§10 管理面）
- 契约: docs/design/gui-contract.md §18（好友角色管理，authz S3）
- 缺口: §18 明确「自定义角色经 CLI `authz role create` 创建（GUI 本轮不做创建入口）」——
  GUI 只能绑定既有角色，无法指定角色权限（创建/编辑/删除自定义角色）。
- 既有能力:
  - crates/p2p-authz: create_role / delete_role / list_roles / show_role / bind / unbind / check；
    **无 update_role**。Permission::registry() 提供闭集枚举。
  - apps/gui/src-tauri/src/authz.rs: 只透出 role_list/bindings/bind/unbind/check/default_role。
  - GUI: FriendRoleDialog（绑/解绑/默认角色），无角色管理入口。
- 门禁: make check（fmt/line-limit≤300/clippy/test/gui-check/gui-tauri-check/panic-hygiene/
  protocol-registry/cli-parity/ai-docs-sync）；新增 GUI 命令必须在 scripts/check/cli-parity.tsv
  登记（mapped 或带理由 exempt），否则 cli-parity 红。
- 踩坑对齐（self-evolving references）: TSX 行内禁 CJK；i18n zh/en 键同步 + 独立小提交；
  严格键类型只有 build 才查；src-tauri 不在根 workspace（gui-tauri-check 单独门禁）；
  authz 判定测试注意内建阶梯并集=闭集九 key。

## 2. 契约（逐字钉死，两卡共用，禁改名）

新增 4 个 tauri command（snake_case；Rust 参数 snake_case，前端 invoke 传 camelCase key）:

    authz_permissions_list() -> { permissions: string[] }        // Permission::registry() 顺序
    authz_role_create(role_id, name, permissions, note) -> { role: AuthzRoleView }
    authz_role_update(role_id, name, permissions, note) -> { role: AuthzRoleView }
    authz_role_delete(role_id) -> { roleId: string }

前端 IPC 接口方法（ipc-types.ts，逐字）:

    authzPermissionsList(): Promise<AuthzPermissionsReport>;
    authzRoleCreate(roleId: string, name: string, permissions: string[], note: string): Promise<AuthzRoleMutationReport>;
    authzRoleUpdate(roleId: string, name: string, permissions: string[], note: string): Promise<AuthzRoleMutationReport>;
    authzRoleDelete(roleId: string): Promise<AuthzRoleDeleteReport>;

新增类型（ipc-types.ts §18 注释块内追加）:

    export interface AuthzPermissionsReport { permissions: string[] }
    export interface AuthzRoleMutationReport { role: AuthzRoleView }
    export interface AuthzRoleDeleteReport { roleId: string }

p2p-authz 新 op（ops.rs）:

    pub fn update_role(&self, role_id: &str, name: &str, perm_keys: &[String], note: &str) -> Result<Role, AuthzError>

语义: 仅自定义角色可改（builtin → 显式 AuthzError 拒；未登记 → RoleNotFound）；
permissions 走既有 parse_perms 闭集解析去重；role_id 不可变（改 id = 删+建）；
成功后 save_roles(custom_roles) 落盘。

校验语义（错误一律可读中文 Err 上浮，禁静默）:
- create: role_id 走既有 validate_role_id（^[a-z0-9-]{1,32}$），与内建四冲突拒（既有行为）。
- update/delete: 内建角色拒改/拒删（既有 delete 已拒自定义外角色则复用）。
- delete 前置两道引用检查: ① 绑定引用（既有 RoleReferenced）；② **加好友默认角色**
  —— authz_role_delete 命令层先比 state.config_get().authz_default_role，相等则
  Err（「该角色是加好友默认角色，请先更改默认角色」），防悬空默认角色
  （对齐 default_role_save 的存在性校验语义；core 层不管配置，闸放命令层）。
- permissions 后端不设最小项数（与 CLI 对齐）；GUI 前端校验 ≥1。

cli-parity.tsv 追加四行:
- authz_role_create  mapped  `authz role create`
- authz_role_delete  mapped  `authz role delete`
- authz_role_update  exempt  CLI 无 role update 子命令，本轮 GUI 先行，CLI 对等后续轮补
- authz_permissions_list exempt 闭集静态只读枚举；CLI 侧 role create 表外 key 报错已携带可用 key 清单

## 3. 任务分解（2 卡并行，文件所有权零交集）

| 卡 | 类型 | 分支 | worktree | 范围（允许触碰） |
| --- | --- | --- | --- | --- |
| TA | backend(rust) | feat/wra-backend | .worktrees/wra-backend | crates/p2p-authz/src/{ops.rs,ops_tests.rs,errors.rs?}、apps/gui/src-tauri/src/{authz.rs,lib.rs}、scripts/check/cli-parity.tsv、docs/design/gui-contract.md |
| TB | frontend | feat/wra-frontend | .worktrees/wra-frontend | apps/gui/src/{lib/ipc-types.ts,lib/ipc.ts,lib/mock-authz.ts,stores/authz-store.ts,views/contacts/role-manager-dialog.tsx(新),views/contacts/role-manager-dialog.test.tsx(新),views/contacts/friend-role-dialog.tsx,i18n/types,i18n/locales/*} |

依赖: TB 消费上述契约（invoke 名/形状已逐字钉死）→ 可并行，集成点后移到主树合并后
全量门禁。i18n 键块预分配: TB 独占 contacts.authz.manager.* 键块。
合并顺序: 先 TA 后 TB（零文件交集，TB rebase 主干后即并）。

## 4. UI 决策（主控定死，TB 无裁量）

- 入口: FriendRoleDialog 角色下拉区块加「管理角色…」次级按钮 → 打开 RoleManagerDialog。
- 新文件 views/contacts/role-manager-dialog.tsx（≤300 行；超了拆 role-editor 子组件文件）:
  角色列表（name + builtin 徽章 + 权限 key 徽章）+「新建角色」+ 行内编辑/删除
  （builtin 行无编辑/删除）；表单: roleId（仅新建可填）、name、note、权限复选框组。
- 权限复选框数据源 = ipc.authzPermissionsList()；标签映射 const
  PERM_LABEL_KEYS 覆盖 §4 九 key（i18n 键 contacts.authz.manager.permChatSend 等），
  未知 key 兜底显示原始 key（闭集扩充后 UI 不阻塞）。
- store: authz-store 增 permissions: string[] + createRole/updateRole/deleteRole actions
  （成功本地推进 roles；失败 console.error + toast 原样上抛，沿既有三态纪律）。
- mock-authz.ts: 内存实现 4 新命令 + 沿既有 seed 注入先例扩展，测试复位齐备。

## 5. 验收与合并

- TA: cargo test -p p2p-authz 绿（含 update_role 新测试四态）+ bash scripts/check/gui-tauri.sh 绿
  + bash scripts/check/cli-parity.sh 绿 + 契约文档增补。
- TB: pnpm -C apps/gui lint + build + vitest run 全绿（退出码证据）+ 新测试覆盖
  列表/建/改/删/内建禁改/表单校验/错误三态 + hardcoded-copy 零 CJK。
- 权限相关改动（红线）: 主控亲自核每卡 diff + 主树全量 make check 后才合并。
- 收尾: ff-only 合并（先 TA 后 TB，各先 rebase origin/main）→ 清 worktree/分支 →
  账本闭环 → SESSIONS.md 释放。

## 6. 遗留观察

- .worktrees/wtd-gui: 上一波（已释放）残留，干净、无独立分支——本波收官时顺手清理。
