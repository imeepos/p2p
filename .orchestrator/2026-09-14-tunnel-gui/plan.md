# tunnel GUI 操作面波 · 计划 v1（2026-09-14）

## 目标（用户指令）
「分享任意 http/ws 协议给另一个节点使用的功能」需要在 GUI 中可以操作。

## 事实基线
- tunnel 泛化波（session-89ef72dc）已收官并释放 scope；后端与 CLI 面完整
  （Serve/Connect、/p2p-base/tunnel/1、gui-contract §19）。
- GUI 已有部分面：remote-access 页 = GenericTunnelCard（访侧开隧道）+ TunnelServeCard（被访 serve）。
- 疑似缺口（待 TG1 证实）：ws 显式支持/文案、serve 侧按 peer 准入授权、分享向导流、
  多服务并存管理、入口可发现性。

## 任务图
TG1（architecture 只读调研 → gap-matrix.md）→ TG2（frontend，按提案实现纯前端项）→
（若 §4 提案含 src-tauri 缺口）TG0（backend，src-tauri 命令增量，独立任务书）→ TG3（code-quality 评审）。

## 预分配
- 分支前缀 feat/tgui-*；i18n 键块 remote-access.tunnel.* 域独立提交。

## 全局约束
同 agent-chat-uix 波（AGENTS.md worktree 协议/行数红线/i18n 双语/无 emoji/测试同提交）；
并存 lead：session-cd1cdce3（禁触 settings/services-card 服务开关面——tunnel 的服务总控
开关在其域内，GUI 复用既有开关状态不改动）。

## 锁定区
TG1（已派发 → session-bf46ed89）。TG2/TG0/TG3 待 TG1 产出后细化派发。

## 待细化区
- TG0 是否开：触发器 = TG1 §4 提案列出 src-tauri 级缺口。
- 入口/命名改善（remote-access 页命名与导航可发现性）：并入 TG2 或单独小任务，TG1 给建议后定。
