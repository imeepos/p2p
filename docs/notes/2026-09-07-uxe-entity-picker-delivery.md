# UX-E 关联选择器 · 交付报告（feat/uxe-entity-picker @ 0fe0e5a）

> 派单：docs/notes/2026-09-07-ux-polish-dispatch.md（F03/F11/F14/F23/F24/F25）
> 状态：完成，已 push origin。main 未动，待机械验收后统一合并。

## 分支
- tip：`0fe0e5a`（origin 已确认 push 成功）
- 提交序列：cd7f142(i18n 键块) → 2466a9c(picker 组件族) → 082ebc0(F03/F24+?add=) → 7492347(F11/F14+?dial=) → 1d16cdf/1cf5a35(F23) → 7875ba8(F25) → 0fabb0c/b4a2ba9(门禁收口+TDZ 真修复) → 8b0b765(反向同步 origin/main d238d23，locale 冲突双侧保留) → 0fe0e5a(skill 喂回)
- 变更：29 文件 +1491/−242；全部 ≤275 行；路由/menu 未动

## 四项门禁（合并 origin/main 后终验）
- vitest：170 文件 / 1008 用例全绿（基线 157/919）
- eslint：0 错 0 警
- tsc -b：0 错
- check:i18n：PASS（zh=en=1041）

## 逐 finding 实测（VITE_MOCK_IPC=1，port 5177 隔离 cacheDir，截图 /tmp/uxe-1~10-*.png）
- F03：好友 PeerId 上方「从发现清单/节点表选择」统一选择器，选中即回填，自由文本兜底可改写（uxe-1/2）
- F24：行内校验 aria-invalid + aria-describedby → role=alert 节点播报，昵称/地址行同口径（uxe-3）
- F11：拨号目标结构化三段（PeerId/地址/端口 + QUIC/TCP），发现带入预填，地址缺传输前缀按「可拆才带入」降级（uxe-5）
- F14：端口失焦即时 role=alert「端口需为 1-65535 的整数」+ aria-invalid + 提交禁用，失焦前不打断（uxe-6）
- F23：群邀请统一多选，已选区置顶 chip「已选 2 项」+ 计数 aria-live，即时搜索过滤；越界沿用 group.manage.inviteOverCap 现有口径（uxe-7/8）
- F25：WS 地址主字段默认展开可见，发现清单选择降辅助填充，peer 自由文本保留高级区兜底（uxe-4）
- URL 契约：#/network/peers?dial=<目标> 三段精确预填（uxe-9）；#/contacts?add=<peerId> PeerId 预填（uxe-10）

## 备注
- 走查当场抓到真 bug：拨号弹窗迁移播种块 TDZ 崩溃（b4a2ba9 修复）
- 走查发现 GroupView 为孤儿组件，群管理真实入口在 contacts 群组分区（已喂回 skill）
- 审计原文 docs/notes/ux-audit-20260907.md 与任务摘录核对一致，无出入
