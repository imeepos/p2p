# TB 任务书：authz 角色管理前端（角色权限管理 UI 全链路）

> 类型: frontend | 分支: feat/wra-frontend | worktree: /Users/imeepos/ext512/p2p/.worktrees/wra-frontend
> 派发者显式 id: session-31ed5fb1-8ca8-4500-9ee6-1fa8a9c1c81d（send_parent 解析异常时定向回投）

## 目标

GUI 中可指定角色权限: 新增 RoleManagerDialog（自定义角色创建/编辑/删除，权限闭集
勾选），入口挂在 FriendRoleDialog；贯通 ipc-types/ipc/mock/store/i18n/测试。
用户痛点: 现状角色只能在 CLI 建，GUI 角色下拉只能选不能管。

## IPC 契约（逐字，禁改名；与 TA 卡共享）

只做 plan.md §2 所钉契约。工作树内读 `.orchestrator/2026-09-12-authz-role-gui/plan.md`。
四个新方法加进 ipc-types.ts 的 DshIpc 接口与 ipc.ts invoke 面（snake_case invoke 名，
参数 camelCase）；mock-authz.ts 同签名内存实现（沿 §18 mock 先例，含 seed 注入扩展与
测试复位）。mock 语义: create 校验 role_id 正则与内建冲突、update 拒 builtin、delete
有绑定引用即 throw 可读错误——与后端校验对齐，测试才可信。

## UI 决策（主控已定死，无裁量）

- 入口: friend-role-dialog.tsx 角色下拉区块加「管理角色…」次级按钮（variant outline,
  尺寸小）→ 打开 RoleManagerDialog。该文件现 256 行，追加后必须仍 ≤300。
- 新文件 views/contacts/role-manager-dialog.tsx（≤300 行；逼近上限就拆
  role-editor-fields.tsx 子组件）: 
  - 角色列表: 每行 name + builtin 徽章（contacts.authz.builtinTag 复用或新建）+
    权限 key 徽章串 + 行内「编辑」「删除」（builtin 行两者皆无）；
  - 「新建角色」按钮切表单态: roleId（仅新建可填，placeholder 注明 ^[a-z0-9-]{1,32}$）、
    name（必填）、note（可选）、权限复选框组（≥1 项，前端校验）；
  - 删除走 useConfirm 确认（destructive），错误原位 + toast 双通道（沿既有纪律）。
- 权限复选框数据源 = ipc.authzPermissionsList()（store 持有 permissions: string[]）；
  标签映射 `const PERM_LABEL_KEYS` 覆盖九 key: chat.send/chat.attachment/a2a.discover/
  a2a.invoke/acp.session/acp.execute/llm.borrow/repair.diag/repair.fix → i18n 键
  contacts.authz.manager.permChatSend / permChatAttachment / permA2aDiscover /
  permA2aInvoke / permAcpSession / permAcpExecute / permLlmBorrow / permRepairDiag /
  permRepairFix；映射外 key 兜底直接显示原始 key（闭集扩充 UI 不阻塞，勿用 t() 包裸 key）。
- store（stores/authz-store.ts）: 增 permissions: string[]（loadAll 并行拉取）+
  createRole/updateRole/deleteRole actions；成功本地推进 roles（delete 后若
  defaultRoleId 指向被删角色，本地回退 "friend" 并 console.error 留信号）；失败
  console.error + 原样上抛。行数控制: 现 88 行，预计 ~180，超 300 拆 store 文件。

## i18n

- 键块独占: contacts.authz.manager.*（你专属，append 到 zh-CN.ts / en-US.ts 的
  contacts.authz 块内尾；zh/en 键集合机械一致；老键不动）。
- TSX 行内零 CJK（hardcoded-copy 门禁连正则里的中文都拦）；i18n types 同步注册
  （独立小提交: 先提 locale 键与类型，再提视图，防 tsc 中间态红——2026-09-03 先例）。

## 输入（读这些，不猜）

- apps/gui/src/lib/{ipc-types.ts,ipc.ts,mock-authz.ts} §18 既有段（形状与 mock 先例）
- apps/gui/src/stores/authz-store.ts、views/contacts/friend-role-dialog.tsx（入口挂点）
- 组件参照: views/contacts/chat-friend-edit-dialog.tsx、components/ui/{dialog,checkbox,label,input,select}、
  components/feedback/{confirm-provider,toast,command-error}
- docs/design/gui-contract.md §18（语义参照，TA 卡负责文档增补，你禁触 docs/）

## 验收标准（可机械判定，逐条给证据）

1. `pnpm -C apps/gui exec vitest run` 退出码 0；新增 role-manager-dialog.test.tsx 覆盖:
   列表渲染（内建四+seed 自定义）、创建成功流（填表单+勾权限→列表出现）、编辑自定义
   （改权限集→保存生效）、删除自定义（confirm→消失）、builtin 行无编辑/删除钮、
   表单校验三态（roleId 非法格式 / name 空 / 权限零项）、命令失败错误呈现。
2. `pnpm -C apps/gui build` 退出码 0（严格 i18n 键类型唯一闸口，vitest 不查类型）。
3. `pnpm -C apps/gui exec eslint .`（或仓库既有 lint 口径）退出码 0。
4. 既有 i18n 键同步/hardcoded-copy 测试全绿（如仓库有该测试文件，机械跑）。
5. 新文件 ≤300 行、新函数 ≤60 行（scripts/check/line-limit.sh 口径自检）。
6. mock 会话冒烟: vitest 里走真 mock-authz（禁 VITE_MOCK_IPC 泄漏进 build 的既有护栏
   语义不变，不改 vite 配置）。

## 边界（明确不做）

- 禁触 crates/、apps/gui/src-tauri/、scripts/、docs/；不改既有 IPC 方法签名与既有键。
- 不做角色排序/拖拽、不做权限矩阵判定预览（authz_check dry-run 面留后续轮）。
- 不做内建角色编辑入口（builtin 不可改，红线 1）。

## 提交纪律

- i18n 键+types 独立小提交 → 视图+store+mock 主提交（feat(contacts): ...）→
  测试随主提交。message: `type(scope): subject`，正文写机理。

## 预算与停止条件

- 修复轮 ≤3。早落盘小步写（先 store+mock，再 UI，再测试）。
- 立即 BLOCKED: mock 与契约形状对不上、ui 组件库缺 checkbox 等基础件、
  friend-role-dialog 加入口即超 300 行且无合理拆法。
- 回报用 session_link_send_parent，状态枚举 DONE / DONE_WITH_CONCERNS / BLOCKED /
  NEEDS_CONTEXT，DONE 附每条验收的命令+退出码（贴命令输出尾行即可，不贴全量日志）。
