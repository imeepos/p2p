# UX-R2 修复波 R2F-C 交付报告（docs/更新卡/ACP，2026-09-07）

实现会话：R2F-C（feat/uxr2c-docs-acp）。基线 main @ 64a5857，审计依据
docs/notes/2026-09-07-uxr2-audit-findings.md，产品裁决见 docs/notes/2026-09-07-uxr2-fix-dispatch.md。

## 分支与提交

- 分支：`feat/uxr2c-docs-acp`（worktree /Users/imeepos/ext512/p2p-uxr2c，未合入 main）
- tip：`59372ab`
- 提交清单（可独立 revert 粒度）：
  - `9eed0e0` fix(docs): R2-20 文档内链接给可见反馈并映射跨文跳转
  - `1f642fb` feat(docs): R2-21 长文滚动出返回顶部按钮
  - `93e156d` fix(update): R2-22 已跳过版本行补「取消跳过」入口
  - `a785c8f` fix(acp): R2-23 连接失败指引指向真实编辑路径并加直达深链
  - `59372ab` feat(acp): R2-24 chat agent 形态待应答权限最小可见指示

## 逐 finding 修复说明与证据

### R2-20（P1）/docs 文档内链接点击零反馈 — 已修复

- 新增 `views/docs/docs-links.ts`：href 三分类——五篇内跨文（映射注册表 id，
  README.md 别名 protocol-overview）、外链（http/https）、不可达仓库路径（blocked）。
- 链接点击行为（docs-markdown）：跨文 → 应用内切换目录并回顶；blocked → toast
  提示 + 复制原路径（console.warn 观测信号保留）；外链 → 桌面端（Tauri）复用
  `update_open_release_page` 的系统 opener 通道（该命令带 github.com 白名单，
  白名单外/mock 态失败降级为复制+提示，见「已知边界」）。
- 渲染文本不再裸露相对路径：跨文链接用文档 H1 人话标题、blocked 用去目录短名，
  `title` 属性保留原路径；`data-doc-link` 标注类别供断言。
- 回归单测：`docs-links.test.ts`（解析 6 例）+ `docs-view.test.tsx`（跨文切换、
  blocked 渲染与复制真实交互）。
- 页面实测（/tmp/uxr2c/walkthrough-out.json s1 + shots/s1-*.png）：链接渲染文本
  全部人话化（noRawPathAsText=true）；blocked 链接点击 toast 原文
  「该链接暂不支持在应用内打开/文档路径已复制：../design/wire-protocol.md」；
  跨文点击 H1 由总览篇切至 quickstart 篇、目录 aria-current 同步。

### R2-21（P2）长文页内导航 — 已修复（低成本方案：返回顶部）

- docs-view 右栏滚动超 240px 浮现「返回顶部」浮动按钮（aria-label 走 i18n），
  点击回顶即隐；跨文跳转不误显。
- 回归单测：docs-view.test.tsx（隐藏→滚动显→点击回顶即隐）。
- 页面实测（s2 + shots/s2-backtop-visible.png）：quickstart 篇 scrollTop=900 时
  按钮出现，点击后 scrollTop=0 且按钮消失。

### R2-22（P2）跳过此版本无撤销 — 已修复（取消跳过入口为主，二者取一）

- update-store 新增 `unskipVersion`（清 localStorage 键与状态，与
  skipCurrentVersion 持久化对称）；已跳过行内出「取消跳过」按钮。跳过轻确认
  与取消入口二选一，按派单以取消入口为主，未再加确认。
- 回归单测：about-update-card.test.tsx（跳过前行不存在→跳过→行+持久化在→
  取消后行消失且 localStorage 键清空）。
- 页面实测（s3 + shots/s3-*.png）：skipped-version localStorage "0.2.0"→取消后 null。

### R2-23（P1）ACP 失败指引指向不存在控件 — 已修复

- 文案源修正（zh/en 同步）：
  - `uxgAcp.endpointIncomplete`（error-help.ts 消费，chat 失败卡行内文案）
  - `acp.console.resolveFailed`（console 发现失败 flowNotice）
  - 均改为「通讯录 → Agent → 本机 agent → 编辑」；不再出现「高级设置」「连接卡」。
- 失败卡直达：ConnectFailureNotice 新增「去编辑本 Agent」按钮 →
  `/contacts?agentDetail=<endpointId>` 深链直开通讯录编辑抽屉。
- 深链为只读参数：agent-section 命中已登记端点才开抽屉，未命中 console.warn
  留观测信号；不改任何现有行为；渲染期同步（react-hooks 纪律，同 contacts-view
  hash 深链先例），未给 agent-detail-drawer 加任何参数。
- 回归单测：agent-conversation-guide.test.tsx（新文案断言 + 编辑链接 href）、
  contacts-agent-flow.test.tsx（深链开抽屉 + 未命中不开）。
- 页面实测（s4 + shots/s4-*.png）：坏凭据 endpoint 连接失败卡文案原文
  「连接失败：缺少 Token 或 Peer ID：请在「通讯录 → Agent → 本机 agent → 编辑」补全…」
  （pointsToRealPath=true、noStalePointer=true），点编辑直达后抽屉打开且
  连接编辑表单（contacts-drawer-edit）在场。

### R2-24（P1，按产品裁决）chat agent 形态权限待应答最小可见性 — 已修复

- 不回挂 AcpView、不删死代码。store 已消费 request_permission 事件的
  `interactions`（interaction-model addPermission）派生 pending 计数：
  chat agent 在线形态计数值 >0 时出 warning 指示条（计数文案）+「去处理」
  按钮经 agentDetail 只读深链直达 contacts Agent 详情权限面板（面板已存在）；
  计数为 0 时不渲染任何节点（零痕迹）。未新增任何交互范式。
- 注入路径（派单预案）：mock 回放脚本无权限步、浏览器 mock 亦无伴生 console
  （真机 8787 与 mock token 不一致，实测 1006），自然路径不可达。故在
  `VITE_MOCK_IPC=1` 下挂 window 注入入口（生产构建不暴露）：
  `__acpInjectPendingPermission()`（登记形状与 store-events 消费帧一致）与
  `__acpInjectAgentOnlineSession()`（在线会话态）。
- 回归单测：agent-permission-banner.test.tsx（无待应答零痕迹/有则计数+深链/
  approved 不算待应答/注入路径可用）。
- 页面实测（s5 + shots/s5-*.png）：注入前零痕迹（zeroTraceBeforeInject=true）；
  注入后指示条「有 1 条待应答权限请求」+ action href
  "#/contacts?agentDetail=acp-local-agent"；点击后抽屉打开、权限面板行在策
  略块内（「Execute command (injected for walkthrough)…等待处理」）。

## 门禁摘要（卡内自查，apps/gui）

- `pnpm lint`：通过（eslint 无输出）
- `pnpm typecheck`：通过（tsc -b 无输出）
- `pnpm vitest run`：Test Files 185 passed (185) / Tests 1098 passed (1098)
- `pnpm build`：✓ built（chunk 体积警告为既有基线，非本卡引入）
- `pnpm check:i18n`：i18n-diff PASS（zh=1178 en=1178）

## 页面实测环境与证据

- 走查实例：vite dev @ http://127.0.0.1:5179（程序化 createServer 复用仓库
  vite.config.ts，cacheDir /tmp/uxr2c/.vite，shell 显式 VITE_MOCK_IPC=1，
  unset 代理 + NO_PROXY=*）；Chrome headless 调试端口 9239，单会话五场景
  顺序执行，收尾进程已清。
- 证据：/tmp/uxr2c/walkthrough-out.json（五场景 DOM 断言全 ok）+
  /tmp/uxr2c/shots/s1…s5 共 10 张截图；走查器 /tmp/uxr2c/dev-server.mjs、
  /tmp/uxr2c/scenario.mjs（仓库零改动）。

## 已知边界与备注

1. 外链系统打开复用 `update_open_release_page`（github.com 白名单）。当前
   五篇文档零外链，实链无影响；若日后文档出现非 github 外链，桌面端会降级
   为复制+提示而非直接打开——需要通用 opener 白名单命令时属契约变更，另立卡。
2. R2-24 页面实测为注入态（派单预案内）：mock 回放脚本无权限步、浏览器态无
   伴生 console 会话，自然触达需真机终验（协调者「真实流程终验」单独卡）。
3. 本卡发现并绕过：5179 端口被 9 月 2 日遗留孤儿进程 /tmp/report-server.py
   （PID 93813，PPID 1）占用，已清；5178 之后的端口登记建议纳入收尾巡检。
4. zh locale 中 `contacts.endpoint.advancedToggle`（添加 Agent 对话框「高级
   设置」折叠）为在用的真实控件，与失败文案无关，未动；llm-share 域引用
   「连接卡」的两条键（needAdmin/needConsole）属 R2F-A 所有权，未触碰。
