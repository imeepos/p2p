# UX-J 地址表单与校验 · 交付报告（feat/uxj-form-labels）

> 派单：docs/notes/2026-09-07-ux-polish-dispatch.md（F13/F14）
> 状态：完成，已 push origin。main 未动，待机械验收后统一合并。
> 审计原文：docs/notes/ux-audit-20260907.md（F13/F14 逐字核对一致）

## 分支
- tip：见 git log（本文落盘前的提交序列如下）
- 提交序列：f7e29ca(i18n 键块) → 76f1c2d(F13 行级标签+弹窗标签) → f8ad523(F14 端口失焦即时校验) → merge origin/main → docs(skill 喂回) → docs(本报告)
- 变更文件（main..HEAD）：
  - apps/gui/src/i18n/locales/zh-CN.ts / en-US.ts（common.addressList.rowLabel、discovery.rendezvous.addrLabel，各 +2 键，独立小提交）
  - apps/gui/src/views/shared/address-list-editor.tsx（行级可见序号标签 + htmlFor/id 关联）
  - apps/gui/src/views/network/discovery/add-address-dialog.tsx（可见字段标签 + aria-invalid/describedby + role=alert）
  - apps/gui/src/views/settings/network-card.tsx（quic/tcp 端口失焦即时校验）
  - apps/gui/src/views/settings/advertise-card.tsx（观测端口失焦即时校验）
  - 配套测试：add-address-dialog.test.tsx（新）、settings-port-blur.test.tsx（新）、advertise-card.test.tsx（新）、relay-config-card.test.tsx（+F13 用例）、settings-focus-error.test.tsx（alert 汇总断言适配）
  - .agents/skills/self-evolving/references/known-issues.md（UX-J 喂回两条）

## 四项门禁（merge origin/main 后终验）
- vitest：173 文件 / 1017 用例全绿（基线 170/1008，净增 3 文件 9 用例，零失败）
- eslint：0 错 0 警
- tsc -b：0 错
- check:i18n：PASS（zh=en=1043，基线 1041）

## 逐 finding 实测（VITE_MOCK_IPC=1，dev server port 5181，CDP 调试端口 9247 专属副本，截图 /tmp/uxj-1~5-*.png）

### F13 地址输入可视标签 —— 通过
- 中继页（#/network/relay）：两条中继地址行各有可见标签「地址 1」「地址 2」，label[for] 与 input[id=relayAddrs-row-N] 一一关联；占位符保持示例 192.168.1.10/u3403（uxj-1）。
- 发现页添加引导地址弹窗（#/network/discovery → 添加地址）：输入框上方可见标签「引导地址」htmlFor 关联；占位符只放示例 192.168.1.10/u3400（uxj-2）。
- 设置页（#/settings）：「宣告地址」「观测地址」组标题下新增行各有可见标签「地址 1」，label[for] 关联 input[id=advertisedAddrs-row-0 / observationAddrs-row-0]（uxj-3）。
- 实现口径：审计允许「可见 label（如中继地址 1）或字段组标题+序号」，采用后者——AddressListEditor 组标题保留，每行渲染「地址 {{index}}」小号可见标签，中继/宣告/观测三个列表同文件受益。

### F14 端口失焦即时校验 —— 通过
操作路径（每项均在 mock dev 单会话内闭环）：
1. QUIC 端口输入 99999、未失焦：无任何提示（首次失焦前不打断输入）。
2. 失焦：就地出现 role=alert「端口需为 0-65535 的整数」（id=settings-quic-port-error），input aria-invalid=true 且 aria-describedby 指向该节点（uxj-4）。
3. 改为 3400：提示即消、aria-invalid 移除，无需失焦或保存（即改即消）（uxj-5）。
4. TCP 端口 99999 同路径复现提示、改 3401 即消；观测端口 99999 失焦出「观测端口需为 1-65535 的整数，或留空」、改 3402 即消。
- 口径对齐既有 dial-target-field 的 F14 实现（role=alert + aria-invalid + describedby）；保存条汇总提示（保存时机）为基线既有行为，本轮未动。
- 「保存前禁用或高亮未决错误」取高亮分支：未决错误在字段就地红字 + aria-invalid 红边高亮，保存动作仍由基线的提交校验兜底。

### 走查环境备注
- headless Chrome 无窗口焦点时程序化 blur() 不派发 React 需要的事件对（探针实测），走查以冒泡 focusout（与真实用户点击别处相同的 React 委托路径）模拟失焦，结论不受影响；已喂回 skill。
- app 错误缓冲仅两条 data-watch 降级日志（纯浏览器环境无 Tauri invoke，2026-09-03 诊断面裁决的既有行为），与本轮改动无关。

## 边界披露
- address-list-editor.tsx 为共享组件，但当前仅中继卡与宣告卡两个本任务页面引用，行级标签为纯增量渲染，不影响其他页面。
- settings-focus-error.test.tsx 非本轮新增文件：字段级 role=alert 加入后保存条汇总断言从 findByRole("alert") 改为 findAllByRole + 文本过滤，断言语义不变。
- settings-view 主体、保存条、menu/路由均未动。

## 备注
- worktree .worktrees/feat-uxj-form-labels 保留待协调者机械验收；合并后按收尾四步清理。
