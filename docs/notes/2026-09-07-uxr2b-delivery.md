# UX-R2 修复交付·R2F-B 监控族与全局一致性打磨（2026-09-07）

分支：`feat/uxr2b-monitor-global`（worktree `/Users/imeepos/ext512/p2p-uxr2b`，基线 main @ 64a5857，已反向同步）
tip：`35428dc`（代码终态；本报告提交为其后最终 tip）。不合并，待协调者机械验收后 ff-only 合入。

## 提交清单（均可独立 revert）

| commit | 内容 |
|---|---|
| f9df9e4 | R2-17 PeerId 缩略统一共享组件，概览/面板与事件页同口径 |
| bd3db83 | R2-16 事件详情原始负载默认折叠并补复制详情 |
| 8540767 | R2-18 趋势 sparkline aria-label 补当前值 |
| 18a1877 | R2-25 面板快捷键提示与 rail 数量同源并清死键 |
| 89fcdb5 | R2-26 console status 不可达在非 Tauri 态降级单次提示 |
| 967131f | R2-19 数据链路横幅补引导失败注入闭环用例（选做，已做） |
| 3c9445b | 事件详情复制测试补 writeText 形参类型（tsc 严格元组） |
| 35428dc | R2-17 概览最近事件截断断言同步 6+4 口径（f9df9e4 漏提交的期望同步） |

## 逐 finding 说明与证据

### R2-16（P2）事件页详情 JSON 默认折叠 +「复制详情」— 已修

- `event-row-detail.tsx`：原始负载不再直排；新增「展开 JSON / 收起 JSON」切换
  （aria-expanded）+「复制详情」按钮，对照诊断页 F27 口径；复制经 `copyText`
  成功 toast「详情已复制」、失败 toast 可见，不静默。
- i18n：`events.detail.{showJson,hideJson,copyDetails,copied}` zh/en 同步新增。
- 回归：`event-row.test.tsx` 4 条新断言——详情展开后 JSON 不可见、切换可见性
  与 aria-expanded、复制写入完整可解析 JSON、收起再展开内容一致。
- 页面实测（5178 mock 实例，gui-agent eval）：`jsonVisibleBefore=false →
  jsonVisibleAfter=true`，复制写入 `JSON.parse(copied).type="node_started"`，
  toast「详情已复制」，截图 `/tmp/uxr2b/shots/r2b-events-detail.png`。

### R2-17（P2）PeerId 截断统一（本卡核心）— 已修

- 排查结论：全站共四处 PeerId 截断——事件页 6+4（F02 shortPeerId）、概览最近
  事件前 8（eventSummary 缺 peerLabel 时的回退）、命令面板节点项前 16、
  概览状态卡/详情抽屉/节点表前 12 族。
- 共享组件：新增 `views/shared/peer-id-short.tsx`（PeerIdShort），内部复用
  R1 F02 产物 `lib/peer-name.ts#shortPeerId`（前 6…后 4），不另造常量；
  title 悬停完整 ID。既有 helper 默认输出零改动（向后兼容）。
- 本卡所有权内三处对齐：概览最近事件行接入与事件页同一
  `usePeerNameLabel`（好友名 + 前 6…后 4）；命令面板节点项换 PeerIdShort
  （整行点击复制语义保留）。复制 affordance 沿用既有形态，不新增视觉元素。
- 所有权外遗留（如实报告，未动）：`views/shared/peer-id-cell.tsx`（节点表，
  点击展开式前缀 12）、`overview-status-cards.tsx` 身份卡与
  `peer-detail-sheet.tsx`（前缀 12 + CopyButton）不在本卡文件所有权内
  （派单表仅授予 overview 趋势/最近事件卡）。三处均已有 hover/copy/展开
  affordance，建议后续卡以 PeerIdShort 统一替换。
- 回归：`peer-id-short.test.tsx` 新增；`recent-event-line.test.tsx`、
  `overview-view.test.tsx`、`command-palette.test.tsx` 截断口径断言更新。
- 页面实测：概览最近事件行 `发现节点 pDZ54P…tqXe（192.168.42.225/35496）`
  （原为 8 位前缀），与事件页同口径。

### R2-18（P2）趋势 sparkline 读屏可达 — 已修

- `overview-trend-card.tsx`：aria-label 由仅指标名补齐为「指标名，当前 N」
  （复用卡内「当前 {{count}}」同一 i18n 模板）；role=img 由 Sparkline 组件
  提供、全部 svg 带 aria-label。审计口径「4 个 svg 半数不可达」按现状收敛为：
  趋势区每个 sparkline svg 必须 role+aria-label 且含当前值。
- 回归：`overview-trend-card.test.tsx` DOM 断言——getByRole("img"，name
  "活跃连接，当前 5"）精确命中 + 遍历全部 svg 必须有 aria-label。
- 页面实测（启动节点后）：`trendAria = ["活跃连接，当前 1", "中继会话，当前 0"]`。

### R2-25（P2）面板快捷键提示同源 + 死键清理 — 已修

- `command-palette.tsx`：底部提示改 `t("palette.hints.navigate", { count:
  MENU_ENTRIES.length })`，与 `useNumberRouteHotkeys` 的越界上界同一来源
  （menu.def 注册数），rail 增减不再产生过期「1..4」。
- i18n：`palette.hints.navigate` 模板化（{{count}}，zh/en 同步）；无引用死键
  `palette.hint`（Cmd/Ctrl+1..8）zh/en 删除，grep 全仓仅剩注释提及；
  check:i18n PASS（zh=1173 en=1173）。
- 回归：`command-palette.test.tsx` 底部提示断言改为按 MENU_ENTRIES.length
  推导，并断言旧「1..4」文案不存在。
- 页面实测：面板底部实文「Cmd/Ctrl+1..6 切换一级入口」。

### R2-26（P2，console-watch 部分）非 Tauri 态降级 — 已修

- `console-client.ts`（console-watch 链路文件）：status 面 fetch 失败的
  warn 改为环境感知——非 Tauri（浏览器 mock/预览）单次 `console.info`
  （capability absent 非错误，F26 control-bridge 同口径），后续静默；
  Tauri 态逐次 warn 保留可观测。折化返回值语义（null/空清单）不变。
  llm-share warn 属 R2F-A，未触碰。
- 回归：`console-client.test.ts` 两条——非 Tauri 多次失败仅 1 条 info 且
  0 warn；置 `__TAURI_INTERNALS__` 后逐次 warn 保留。
- 页面实测：全新会话 `errors` 采集，acp 相关 console 行 0 条
  （修复前每会话固定 1 条「console status 不可达」warn）。

### R2-19（P2，选做）引导失败注入 e2e — 已做

- `data-link-banner.test.tsx` 新增闭环用例：ipc 层注入 nodeStatus/metricsGet
  失败驱动真实 `bootstrap()` → 断言 role=alert 横幅 + 失败原因 + 重试按钮；
  注入恢复后点击「重试」走真实 bootstrap → ready → 横幅自动消失。

## 门禁摘要（卡内自查，apps/gui）

| 门禁 | 结果 |
|---|---|
| pnpm lint | 通过 |
| pnpm typecheck | 通过 |
| pnpm vitest run | 184 文件 / 1090 用例全绿 |
| pnpm build | 通过（chunk 体积告警为存量，非本卡引入） |
| pnpm check:i18n | PASS（zh=1173 en=1173） |

## 走查环境与收尾

- vite dev @ 5178（VITE_MOCK_IPC=1 shell 显式注入，unset 三代理 +
  NO_PROXY='*'，worktree 独立 cacheDir）；gui-agent 副本调试端口 9238。
- 收尾已杀净：5178/9238 端口归零，无 gui-agent/Chrome 残留。

## 备注

- `use-hotkeys.ts` 文件头注释仍写「Cmd/Ctrl+1..4」（hooks/ 不在本卡所有权），
  行为实现是 MENU_ENTRIES.length 上界，无用户可见影响，留给后续顺手清。
- event-meta.ts 的 `short()` 8 位回退用于 messageId/groupId（非 PeerId），
  未在本卡口径内，保持原样。
