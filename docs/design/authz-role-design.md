# 统一权限系统（authz）：角色 = 权限，按好友绑定 —— 设计方案 v1

> 状态: v1（拍板记录见 §14） | 日期: 2026-09-09
> 依赖: [acp-over-p2p-design.md](acp-over-p2p-design.md)（策略表/权限瀑布）、
> [a2a-over-p2p-design.md](a2a-over-p2p-design.md)（private agent 授权）、
> [idle-token-sharing-plan.md](idle-token-sharing-plan.md)（llm-share 三闸）、
> [remote-support-plan.md](remote-support-plan.md)（ticket/白名单/审批）
> 定位: 纯加法的授权层。不改通信内核，不替代任何既有执行闸门（监狱/红线/白名单/
> 审批全部保留为纵深防御）；本方案只统一「谁能用哪个域」这一层判定。

## 1. 定位与目标

给每个好友配置一个**角色**，角色 = 一组**权限**；权限按域（chat/a2a/acp/llm/repair）
闭集登记。所有现存与将来的对外授权面统一查同一张绑定表。

1. 统一判定：五个权限面（§2）从各查各表，收敛为查同一个 `p2p-authz` 库。
2. 角色即权限集：owner 配角色，不逐项勾权限；自定义角色 = 自选子集。
3. 默认拒绝不回退：无绑定 = 全域拒绝；存储损坏显式报错（沿策略表先例）。
4. 新业务接入 = 登记权限 key + 入口一处判定，不再各自造表。

非目标：不做行级/对象级 ACL（对象粒度留在各面自己的执行细节表，§8）；
不做 owner 权限的授予（§11 红线）；不改线协议。

## 2. 现状盘点：五张互不相干的授权面

| 面 | 现判定 | 存储 | 粒度 |
|---|---|---|---|
| ACP 桥（/dsh-acp/1 准入） | `PolicyTable.authorize` 默认拒绝 | acp-common policy（peers.json） | peer → scope/mcp/ask_route |
| A2A private agent 任务 | a2a-grants | `<data>/a2a-grants.json` | (agent_id, peer) |
| llm-share 三闸之闸 1 | allowlist HashSet | `<data>/llm-share/allowlist.json` | peer（+模型白名单） |
| repair-helper 票据签发 | mint 本地无前置校验 | 无表（票据即授权） | ticket scope=diag/fix |
| IM 好友簿 | 好友 = 聊天通路，无权限语义 | chat/friends.json（yrs） | peer |

问题：同一个人在不同面要分别授权/撤销，语义互不相通；新面（将来 any）又要再造一张表。

## 3. 模型：Permission / Role / Binding

    Permission   闭集字符串 key，"<domain>.<capability>"，只加不删（废用标 deprecated）
    Role         { role_id, name, permissions: Vec<Permission>, builtin: bool, note }
    Binding      { peer_id, role_id, granted_at, note, expires_at: Option<unix秒> }
    Decision     Allow | Deny { reason: NotBound|Expired|BrokenRole|MissingPerm }

- **角色是权限的集合**，不要求阶梯包含；内建四个（§5）恰呈阶梯便于 GUI 呈现。
- **绑定单值**：peer → 恰一个 role（MVP）。改绑 = upsert；解绑 = 移除条目。
- 角色与好友簿**解耦**：authz 只认绑定表，不 import chat crate（依赖方向：
  p2p-authz 零依赖业务 crate；chat 不感知 authz）。好友簿管「这个人是谁」，
  authz 管「这个人能用什么」。
- 权限**不带参数**：`acp.session` 不含 scope、`llm.borrow` 不含模型——对象级
  参数留在各面既有表（§8），registry 保持闭集简单（§14 拍板 3）。

## 4. 权限登记表（首批闭集）

单一真值源 `crates/p2p-authz/src/permissions.rs` 常量表 + 数据一致性测试锚定
（沿 whitelist_data 先例：文档-表-测试三方对齐，改表即测试红）。

| key | 语义 | 对应现状 |
|---|---|---|
| `chat.send` | 与我双向聊天（含附件） | 好友即通（现状无判定，权限预留） |
| `chat.attachment` | 发送四类附件 | 同上 |
| `a2a.discover` | 发现我的公开 agent 卡 | 公开池本就公开（现状无判定，预留） |
| `a2a.invoke` | 对我的 private agent 发起 A2A task | a2a-grants |
| `acp.session` | 建立对我托管 agent 的 ACP 会话 | acp 策略表准入 |
| `acp.execute` | ACP execute 类工具免问直放 | 权限瀑布 Forward 前置 |
| `llm.borrow` | 经我代理借用上游 LLM 额度 | llm-share 闸 1 |
| `repair.diag` | 可获签 diag 工单 | mint 前置（新增闸） |
| `repair.fix` | 可获签 fix 工单 | mint 前置（新增闸） |

Key 发布规则：新 key 必须先登记本表 + 同步 §8 接入点说明，随常规 PR 评审；
registry 无 owner-only key —— **模型上杜绝经角色提权到 owner**（§11）。

## 5. 内建角色（builtin，不可改，版本化）

| role_id | 权限 | 语义 |
|---|---|---|
| `friend` | chat.send, chat.attachment, a2a.discover | 基础社交；加好友默认绑定的角色（default_role，可配） |
| `guest` | friend + `acp.session` | 能看能问 agent（执行细节仍在策略表，sandbox 语义） |
| `operator` | guest + `a2a.invoke`, `acp.execute`, `repair.diag` | 能操作 agent、跑诊断 |
| `ally` | operator + `llm.borrow`, `repair.fix` | 全面信任；可借额度、可修机器 |

自定义角色：owner 经 CLI/GUI 创建，permissions 取自 §4 闭集任意子集；
`role_id` 命名 `[a-z0-9-]{1,32}`，与内建四名冲突即拒。

## 6. 存储

`<data-dir>/authz/roles.json` 与 `authz/bindings.json` 两文件：

    { "version": 1, "roles": [ Role... ] }
    { "version": 1, "bindings": [ Binding... ] }

- tmp+rename 原子写；损坏/版本不符**显式报错拒读**，禁止静默回退空表
  （沿 acp-common PolicyStore 语义）。
- 写路径先重读磁盘再按主键 upsert 合并，缩小 GUI/CLI 双进程后写覆盖窗口；
  已知局限如实声明，yrs 化（好友簿先例）列为后续演进（§14 开放问题 2）。
- 引用完整性：删角色前检查绑定引用，有引用即拒（先解绑再删）；Binding 悬空
  role 在判定层落 `BrokenRole` 拒绝（防御性，正常路径不该出现）。

## 7. 决策语义（PDP）

    fn check(&self, peer: &PeerId, perm: Permission, now_unix: u64) -> Decision

判定顺序（纯内存，无 IO）：

1. 绑定表无该 peer → `Deny(NotBound)`（默认拒绝）
2. `expires_at` 已过 → `Deny(Expired)`
3. 角色缺失 → `Deny(BrokenRole)`
4. 角色权限集不含 perm → `Deny(MissingPerm)`
5. → `Allow`

时钟注入（Clock trait，沿 repair-enforce 先例）；判定为纯函数可机械测试；
Deny 一律带 reason 码入审计，不泄细节。

## 8. 各面接入（PEP）与保留防线

| 面 | 改造后准入判定 | 保留防线（全部不动） |
|---|---|---|
| ACP 桥 | `PolicyTable.authorize` **且** `check(acp.session)` | scope 监狱/cwd 改写、mcpServers 剥离、TOFU、分享链接一次性 |
| ACP 权限瀑布 | execute 类 ask 前加 `check(acp.execute)`，Deny 即直拒不弹窗 | read/fetch 静态放行、owner-local 拒绝、审批 UI |
| A2A private task | `check(a2a.invoke)` **且** agent 级 grants 表保留双查 | 卡片验签/TTL/版本替换 |
| llm-share | 闸 1 allowlist 换为 `check(llm.borrow)` | 模型白名单、req_id 幂等、并发、预授权冻结 |
| repair mint | `mint(scope)` 前置 `check(repair.diag|fix, bridge_peer)` | 票据一次性/签名/绑对端/红线/白名单闭集/审批状态机 |
| IM 聊天 | 本轮**不接判定**（现状好友即通，行为零变化） | 64MiB 附件上限、文件名净化 |

分层原则：authz 答「这个人能不能进这个域」（peer 级粗粒度）；各面既有表答
「进来之后针对具体对象/参数怎么执行」（agent 级/scope 级/模型级）。双查面
（ACP、A2A）两表各司其职，语义不重叠。

## 9. 迁移

`p2pctl authz import`（幂等可重跑，每面独立子命令）：

| 来源 | 映射规则 |
|---|---|
| acp peers.json | scope=sandbox → 绑 guest；workspace → 绑 operator；Owner 条目跳过（loopback 不进 authz） |
| a2a-grants.json | 有任一 grant 的 peer → 绑 operator |
| llm-share/allowlist.json | allowlist 条目 → 绑 ally（模型白名单留在原处不动） |

切换顺序（逐面一个小提交）：import → 该面判定切 authz → 原表转只读归档
（不删文件，回滚 = revert 提交即恢复旧判定）。已有 peer 的既有条目与角色
绑定冲突时以**更严者**为准（绑定缺失即拒，不因 import 而放宽）。

## 10. 管理面

- CLI：`p2pctl authz role list/show/create/delete`、`authz bind <peer> <role>|--unbind
  [--expires <unix>]`、`authz check <peer> <perm>`（dry-run 判定）、`authz import ...`
- GUI：好友页增「角色」列与编辑对话框（含过期时间）；契约新增章节
  （gui-contract.md §18，实现期独立登记提交）
- 审计：grant/revoke/过期/Deny 事件落既有审计面；角色变更记录 diff

## 11. 红线

1. **owner 不可经角色授予**：registry 不设 owner-only 权限 key，内建角色权限集
   与 owner 能力天然不相交；acp owner scope 仅 loopback 既有路径，authz 不涉足。
2. **默认拒绝**：无绑定 = 拒；authz 读失败 = 拒（显式报错，不静默放行）。
3. **authz 是准入层不是执行层**：撤角色立即影响新判定，已建立的执行中会话由
   各面既有收尾路径处理（ACP 断链、llm 冻结结算、repair 票据自然过期），
   本方案不引入跨面「踢会话」机制（列为后续可选）。
4. 角色/绑定文件**不上 wire**：授权状态只存本机，不经 P2P 同步（防篡改面为零）。

## 12. 分期

- **A1 底座**：crates/p2p-authz（permissions/role/binding/decision + 存储 + Clock）
  全单测；p2pctl authz 管理命令；registry 数据一致性测试。
- **A2 三面接入**：import 幂等导入 → llm-share 闸 1 → A2A → ACP 双查，逐面切换
  逐面提交；每面带「判定切换不回归」对照测试。
- **A3 收尾**：GUI 好友页角色管理（契约 §18 独立登记）、repair mint 前置、
  default_role 配置、审计事件、friends 页角色徽章。

## 13. 与既有机制的关系一览

    请求 → authz.check(peer, domain)     ← 本方案（谁能进）
         → 面内对象/参数表（agent 级/scope/模型/…）  ← 既有（进哪个具体对象）
         → 执行闸门（监狱/红线/白名单/审批/一次性票据） ← 既有（进来怎么执行）

## 14. 拍板记录与开放问题

拍板（2026-09-09 本轮）：
1. 权限闭集 + key 只加不删；角色 = 权限集合；绑定 peer 级单值。
2. 内建四角色呈阶梯（friend/guest/operator/ally）；自定义角色任意子集。
3. 权限不带对象参数，对象粒度留各面既有表（双查分层，§8）。
4. owner 不进模型（红线 1）；授权状态不上 wire（红线 4）。

开放问题（不阻塞 A1/A2）：
1. 好友分组 → 角色模板联动（入组自动绑默认角色）？
2. bindings/roles 是否 yrs 化以支持 GUI/CLI 双进程与未来多设备（好友簿先例）？
3. 撤销是否要求跨面踢会话（红线 3 的加强版）？
4. chat.send/chat.attachment 何时真正接入判定（如「陌生人消息」特性时）。
