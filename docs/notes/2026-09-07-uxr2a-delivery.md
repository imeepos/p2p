# UX-R2A 交付报告：llm-share 域体验打磨（R2F-A 卡）

- 分支：feat/uxr2a-llmshare（worktree /Users/imeepos/ext512/p2p-uxr2a）
- tip：07477f3 fix(llm-share): offer 未发布匹配片段以转义书写,过硬编码 CJK 门禁
- 基线：main @ 64a5857（开工前 merge main 反向同步）；按派单不合并，待协调者验收后合入
- 提交序列（P0→P1→P2→route 注册，均 type(scope): subject）：
  4ec7372 R2-01 / 9478f3b R2-02 / 078b30b R2-03+R2-26 / 4b3d662 refactor / f5dd9a8 R2-04 /
  5accc5d R2-05 / a0115ea R2-06 / 98be4dc R2-07 / a552619 R2-08+09 / 28e91de R2-10+11 /
  9b4934d R2-12+14+15 / 8c3c7ab R2-13(menu.def 独立小提交 chore(gui-route)) / 07477f3 门禁修复

## 逐 finding 状态（17/17 fixed）

| # | 状态 | 修复与证据 |
|---|---|---|
| R2-01 | fixed | 借用真实入账（done/stream_broken，appended=true）经域内 pub-sub（ledger-sync.ts）广播，净差/流水卡订阅重拉；净差卡补「刷新」按钮（balance-refresh）；流水「查询」经 reloadSignal 联动净差。页面实测：借用完成后 balance-row/ledger-row 免手动出现（walk-out.json s5，截图 s5）；拒绝路径不误刷。回归 ledger-linkage.test.tsx 三例 |
| R2-02 | fixed | deny 走 ConfirmProvider（destructive），文案含后果（立即不可借用、默认拒绝）与恢复路径（再次加入）。页面实测：弹窗文案、取消保留、确认回空态（s6 + 截图 s6）。回归 allowlist-panel.test.tsx |
| R2-03 | fixed | offerShow 报错经 isOfferNotPublished 识别未发布语义（offer-errors.ts），空态静默走中文出路文案，不再被 loadError 兜底覆盖成英文原文。页面实测：s1 断言空态含中文标题/出路、不含 never published（截图 s1）。回归 offer-errors.test.ts + offer-panel.test.tsx |
| R2-04 | fixed | 表头 PeerId→i18n「借方」（columnPeer）；主列复用 PeerNameCell（好友昵称+前6后4缩略，title 悬挂全量）；授权时间 formatDateTime 本地化。页面实测：s2 表头/缩略/无 ISO 泄漏（截图 s2）；昵称路径由单测覆盖（chat-store 造好友）。回归 allowlist-panel.test.tsx 两例 |
| R2-05 | fixed | 「借方 PeerId」「出借方 PeerId」接入 EntityCombobox（peer-options.ts，数据面=selectPeerList+好友昵称），自由文本兜底保留；base58/32 字节即时校验复用 isValidFriendPeerId，F14 失焦时机；aria-invalid/describedby 同步。页面实测：s2 非法值失焦即时报错、修正后错误消失、合法值通过（截图 s2）；s1 双选择器在场。回归 peer-id-field.test.tsx 四例 |
| R2-06 | fixed | model 增「从已放行模型选择」选择器（数据源=白名单模型集去重，dial 带入同款），自由输入兜底；maxTokens/messages 补占位+辅助说明（hint id 关联）。页面实测：s1 两控件/占位/说明在场。回归 borrow-panel.test.tsx 两例 |
| R2-07 | fixed | ①重试按钮去 reqId 括注 ②disputeWindowSecs→「争议窗口 N 小时」 ③appended→「已入账/未入账」 ④拒绝态不再渲染内部英文句、预览改标「上游返回片段（截断）」 ⑤拒绝码保留+人话原因+复制详情（CopyButton，F06 先例） ⑥白名单空态标题去 allowlist 字样（设置入口卡同根文案同步） ⑦净差视图/过滤/表头角色术语改「出借方/借入方」 ⑧offer 标签去 model=N 语法片段（示例留占位符） ⑨SSE 帧数→SSE 事件数；请求 ID→请求编号。页面实测：s4 报告卡全文无术语泄漏、争议窗口 24 小时；拒绝路径人话原因+拒绝码+复制详情见走查第二轮输出（borrow rejected report）。回归 borrow-report-terms.test.tsx 四例 |
| R2-08 | fixed | 发布成功 toastSuccess「能力声明已发布，借方可见」。页面实测：s7 toast 文本在场（截图 s7）。回归 offer-panel.test.tsx（toast mock 断言） |
| R2-09 | fixed | 剩余秒数→「剩余时间」人性化：live 显 formatUptime 时长，过期/未生效显状态语义；状态卡拆出 offer-status-card.tsx。页面实测：s7 remaining=「1小时 0分」非裸秒。回归 offer-panel.test.tsx 三例 |
| R2-10 | fixed | 流水表补「时间」列（columnTs，formatDateTime 本地化）+「共 N 条 · 合计 X tokens」统计行（ledger-stats）。页面实测：s5 statsLine=「共 1 条 · 合计 1801 tokens」、时间列本地化（截图 s5）。回归 ledger-panel.test.tsx |
| R2-11 | fixed | 净差行组合值拆带标签明细「出借 X · 借入 Y · N 笔」（balanceDetail 插值）；lender 走 peerDisplayLabel 昵称映射。页面实测：s5 balanceDetailText=「出借 0 · 借入 1801 · 1 笔」。回归 ledger-panel.test.tsx（含好友昵称映射断言） |
| R2-12 | fixed | 三面板校验字段补 aria-invalid/aria-describedby（错误文本节点带 id），新增本卡 focusFirstInvalidField（自管校验版）失败即聚焦第一个错误字段。页面实测：s3 空 Borrow 提交聚焦 llm-borrow-peer、aria 关联齐备（截图 s3）。回归 form-a11y.test.tsx 三例 + focus-first-error.test.ts 两例 |
| R2-13 | fixed | menu.def.ts 注册 /llm-share（Share2 图标，设置前一位，rail 末位沉底约束保持），Cmd/Ctrl+1..7；独立小提交 chore(gui-route)（8c3c7ab），仅注册+图标 import+头注。页面实测：s1 rail 七入口含 #/llm-share（截图 s1） |
| R2-14 | fixed | 二选一取「状态行」方案：busy 期间渲染 role=status/aria-live「正在调用出借方，请稍候…」（AsyncButton 与 form submit 语义冲突，弃用）。回归 form-a11y.test.tsx（deferred promise 断言状态行出现/消失）；页面走查 mock 结算即时无法目击，以单测为准 |
| R2-15 | fixed | 「留存自述」渲染可选输入+用途说明（formRetentionHint），值随发布请求提交。页面实测：s1 字段与说明在场；s7 发布链路含 retention 输入。回归 form-a11y.test.tsx（spy 断言 retention 入请求） |
| R2-26（llm-share 半） | fixed | offer show 未发布空态不再 console.warn，真实错误会话级单次（warnOfferLoadOnce + resetOfferLoadWarnForTest）。页面实测：全程走查 consoleLogs 无任何 llm-share warn（walk-out.json consoleLogs）。[acp] warn 属全局另半，非本卡所有权 |
| — | 不适用 | R2-16…R2-25 归监控族/docs/更新卡/ACP 各卡；R2-24 产品拍板非代码项 |

## 实测证据（mock 走查，/tmp/uxr2a/）

- 环境：程序化 createServer 复用仓库 vite 配置（仓库零改动），cacheDir /tmp/uxr2a/.vite，端口 5177，shell 显式 VITE_MOCK_IPC=1 + unset 代理；Chrome headless 调试端口 9237 + --no-proxy-server；单会话顺序驱动 /tmp/uxr2a/walk.mjs + steps.mjs
- DOM 断言 JSON：/tmp/uxr2a/walk-out.json（s1–s7 + consoleLogs 全量）
- 截图：/tmp/uxr2a/shots/s1…s7 七张（初始 rail 与空态/白名单表/借用 aria 焦点/借用报告/账本联动/移出确认/发布 toast）
- 控制台：全程 llm-share 相关 warn 计 0（R2-26）；错误缓冲无新增
- 收尾：dev server 与调试 Chrome 已全部终止，进程零残留

## 门禁（apps/gui，分支 HEAD 07477f3）

- pnpm lint：exit 0
- pnpm typecheck：exit 0
- pnpm vitest run：exit 0，Test Files 189 passed (189)，Tests 1114 passed (1114)（含本卡新增回归 24 例）
- pnpm build：exit 0，✓ built in 1.75s
- pnpm check:i18n：PASS（zh=1196 en=1196）

## 备注

- 全程未触碰 views/network、views/docs、components/command-palette、components/monitor（仅 import 其 CopyButton 原语）、views/contacts（仅 import 校验规则）、ACP 文件；menu.def.ts 变更仅 R2-13 独立小提交
- i18n 仅增改本卡键（llmShare.*）；设置入口卡 settings.llmShare.entryDescription 因 R2-07⑥ 同根术语一并修正（R2-07 提交内注明）
- 主树 main 在本卡工期内已被并行卡推进（R2F-B 等）；按派单本分支不合并，合入时如遇 i18n/菜单相邻行冲突请在协调者侧消化
