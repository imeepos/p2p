# authz A3 执行计划：GUI 装配 / mint 前置 / 审计 / default_role / 角色管理 / 清扫

> 状态: v1 | 日期: 2026-09-09
> 上游: [authz-role-design.md](authz-role-design.md)（§12 A3 + §14 开放问题）
> 来源: 2026-09-09 复盘（待办清单 + 过程缺陷），社区最佳实践检索结论已内化到各条目

## 0. 社区最佳实践检索结论（内化为约束）

- 审计（[SaaS Audit Logging](https://viprasol.com/blog/saas-audit-logging/)）：授权变更（grant/revoke/
  变更）属"必须记录"；变更记 before/after 快照；append-only；写失败必须可观测（不许静默丢）。
  本仓 llm-share-ledger 已有哈希链先例，审计表沿用其形态精神但 MVP 先 JSONL。
- PEP（[PEP 指南](http://devsecopsschool.com/blog/policy-enforcement-point/)）：判定点必须集中、
  拒绝路径必须留审计信号——与 authz-role-design §8 双查分层一致，不新增机制。
- 缓存（[Authorization-Aware Caching](https://softwarepatternslexicon.com/caching-patterns-and-invalidation/security-and-privacy/authorization-aware-caching/)）：
  授权缓存的失败模式比命中率重要；当前准入频率下"每次判定重读"是最简正确解，
  缓存继续推迟（推翻"下阶段可做"的原判，无实测热路径不缓存）。
- 默认角色（[Birthright access](https://nhimg.org/articles/birthright-access-is-convenient-but-least-privilege-still-sets-the-limit/)）：
  开通时赋予最小默认角色是惯例，但以最小权限为界——default_role=friend（仅聊天域）。

## 1. 执行项（串行，每项独立提交、独立可 revert）

### S1 P0 —— GUI ServeCore 装配 AuthzChecker（llm-share 通电）
- 内容：apps/gui src-tauri ServeCore 组装 LenderProxy 处注入 `with_gate1_authz`，
  Authz 数据根与 CLI/GUI 共用 `<data-dir>/authz/`；装配失败路径显式（读失败=拒+日志）。
- 验收：①GUI 侧装配点单测（有绑定放行/无绑定拒）；②`cargo test` GUI 侧相关套件绿；
  ③llm-share 既有套件零回归；④clippy 零增量；⑤手工冒烟记录（import→check→借调链路）。

### S2 P1a+P1b+P1d —— mint 前置 / 审计事件 / default_role（三个小项一分支三提交）
- **P1a mint 前置**：repair-helper mint-ticket 签发前校验目标 bridge_peer 持有
  `repair.diag`（scope=diag）/`repair.fix`（scope=fix）；无绑定拒且留审计日志；
  一次性票据/签名/过期语义零改动。验收：新增前置判定用例（有绑定过/无绑定拒/
  读失败拒）+ 既有 ticket 套件绿。
- **P1b 审计事件**：`<data-dir>/authz/audit.jsonl` append-only JSONL，事件形态对齐
  社区惯例：`{at, actor:"owner", kind:"authz.role.created|authz.bound|authz.unbound|
  authz.role.deleted|authz.denied", before, after, note}`；grant/revoke/删除/判定拒绝
  五类必记（判定拒绝按面采样：acp 已有 AuthzDenied 即复用，不双写）；写失败
  tracing::error 不阻塞主操作。验收：五类事件各一测 + JSONL append-only 语义测
  （重跑/幂等不重写旧行）。
- **P1d default_role**：配置项 `authz.default_role`（缺省 "friend"，可设其余内建或
  自定义角色 id，设为 "" 表示不自动绑）；p2pctl friend add（及 GUI 加好友后端路径）
  成功后自动 bind default_role，已有绑定跳过；写审计事件。验收：加好友自动绑/
  已绑跳过/显式禁用三测。
- 顺序约束：P1b 先行（P1a/P1d 的审计事件落同一份 sink）。

### S3 P1c —— GUI 好友页角色管理 + 契约 §18
- 内容：Tauri 命令（authz 角色列表/绑定/解绑/默认角色读写，对齐 p2p-cli 已有能力）、
  契约新增 §18 章节（独立登记提交）、GUI 好友页角色列+编辑对话框、i18n、菜单注册。
- 验收：契约守卫测试、GUI 组件测试、i18n key 双语言齐、parity 表更新；
  中央登记文件（menu.def/App.tsx/locales）独立小提交。

### S4 P2 —— clippy 债清扫
- 内容：apps/acp-agent（invites.rs/handler.rs 等 5 处）与 apps/cli（a2a/mod.rs、
  config.rs 等 4 处）独立 workspace 的存量 lint 归零；不碰根 workspace 文件。
- 验收：两独立 workspace `cargo clippy --all-targets -- -D warnings` exit 0；
  根 workspace 测试零回归。

## 2. 串行安排与会话

S1 → S2 → S3 → S4 各一个专属新会话，串行不并行（复盘教训：小体量面并行
的协调税大于墙钟收益）。每个会话：worktree `.worktrees/authz-s<N>-<slug>`、
分支 `feature/authz-s<N>-<slug>`、完工 push + 报告、负责人验收 ff 合并、
主树复证、清理、归档。main 任何写入（含 docs/skill）只由负责人执行。

## 3. 任务书模板修正（过程缺陷修复，对所有会话生效）

1. main 写入穷举禁令：任何 main 操作含 docs/skill 提交一律报负责人。
2. worktree 路径固定 `.worktrees/<name>`，禁止仓库外路径。
3. 自证范围必须列出跨面回归套件（涉及 itest 的面必须跑对应 *_wave）。
4. rebase 后必须 `cargo metadata`/构建一次防 Cargo.toml/lock 重复 key 静默撞。
5. 长时间静默（>30min）需主动投递进度回执。
6. 每完成一个独立可 revert 的变更即刻测试 + commit，不过夜。

## 4. 明确不做（克制）

- 权限决策缓存（无实测热路径，重读即正确解）
- 审计 HMAC 哈希链/告警/保留策略（单机本地文件场景，JSONL append-only 足够；
  哈希链等接多设备同步议题一起评审）
- repair 面 import（无旧表）
- 跨面踢会话、分组→角色模板（维持设计 §14 推迟判定）
- S4 追记（2026-09-09）：GUI 套件负载过敏（高负载下 forks 池启动/1s 级等待超时假红），
  串行跑法为验收口径：`pnpm test:serial`（vitest run --no-file-parallelism）。
