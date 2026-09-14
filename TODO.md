# TODO.md — 需求与边界事实源

> 约定：目标 / 验收标准 / 明确不做的唯一事实源。波次细节见 .orchestrator/<波次>/plan.md。

## 当前波次：2026-09-13 权限统一收口 + 服务总控（service master）

**来源**：用户直接指令（2026-09-13）：「所有权限统一由用户权限模型管理 所有服务均可手动开启和关闭」

**目标**：
1. 权限统一：所有对外能力判定收敛到 p2p-authz 用户权限模型（角色 = 权限闭集，
   按好友绑定），不留散表散闸；缺口能力先登记 key 再接 PEP。
2. 服务总控：所有服务均有用户可手动开启/关闭的开关，GUI 与 CLI 同源同语义。

**验收标准**：
- 能力闸盘点表（file:line 级证据）落盘，无未登记的对外授权面。
- 每个服务有显式开关；开关状态、持久化语义、默认值在契约文档成文。
- GUI 与 CLI 均可枚举服务、查询与翻转开关；cli-parity 登记齐全。
- 全量 make check 绿。

**明确不做**（本波）：
- owner 本机管理操作不做角色授予（authz 设计红线 1，服务开关属 owner 本机面）。
- 不做跨面踢会话（authz 设计红线 3 维持）。

**待用户裁决**（裁决前锁定区不派实现卡）：
- 服务总控落地形态（统一注册表 vs 散点补开关 vs 含热切换）。
- 权限收口范围（chat.send 真接判定？tunnel visit 是否入 authz？）。
- 开关持久化与安全默认语义。

## 微波次：2026-09-14 agent 聊天页按钮反馈（agent-chat-feedback）【已收官 2026-09-14】

> 收官：AF1（新建会话反馈闭环）@ 74276ae3 + AF2（全按钮走查接线 + listWorkspaces
> 吞错契约修复）@ f75e0323 全并，main==origin/main @ f75e0323，check-fast + gui-check PASS。

**来源**：用户直接指令（2026-09-14）：「本机 agent 聊天页面 点击新建会话 大半天没有反馈，
多次连续点击后 进入聊天页面。所有按钮都需要优化：点击-微反馈loading-成功微提示-错误轻提示」。

**目标**：新建会话与聊天页全部异步按钮具备统一反馈闭环：点击即时 pending、成功微提示、
错误轻提示、pending 期间防重复触发。

**验收标准**：
- 新建会话两入口（agent-conversation / session-sidebar）点击即时 spinner+disabled，
  store 单飞防连点；成功/失败 toast 各弹一次（与存量失败 toast 不双弹）。
- 走查清单覆盖 views/chat + acp/components 全部交互控件，异步按钮全部接线，
  渲染矩阵测试覆盖 pending/成功/失败三态。
- 聚焦测试、typecheck、lint 退出码 0；console 零新增报错。

**明确不做**（本微波）：
- 不做 120s 超时包络调整（66c5a147 已覆盖）、服务端防重、会话分组/视觉改版（uix 波规划）、
  settings/remote-access/src-tauri（他波 scope）。

**细节**：`.orchestrator/2026-09-14-agent-chat-feedback/plan.md`（AF1/AF2 并行，
分支前缀 feat/acf-*，接管登记见 .agents/collab/SESSIONS.md）。
