# UIX-SPEC · 工作区→会话侧栏与聊天区设计对齐规格

任务书 T1（architecture · 只读调研）产出。参考实现 = DSH 插件化客户端，基目录
`/Users/imeepos/ext512/ymm-001/deepseek-harness/packages/client`（下文引用省略该前缀）。
本仓 = `/Users/imeepos/ext512/p2p`，tailwind v4 + shadcn 风格 + zustand。

**输入偏差声明**：任务书列出的 `store/src/contract.ts` 实为通用 store 引擎契约
（ObservableSnapshot/StoreSpec，contract.ts:1-144），**不含**工作区/会话数据模型；
真实数据模型在 `ui-workspace/src/client/tree.ts`（SessionNode/GroupNode）与
`ui-workspace/src/client/stores.ts`（视图状态）。本文按 tree.ts 提取，其余路径全部符合任务书。

---

## 1. 设计 token 对照表

dsw 变量/几何值 → 本仓 `apps/gui/src/index.css` 等价物；无等价者标「建议新增」并给
tailwind v4 `@theme` 写法。参考实现 file:line 均为 dsw token 的**定义处或使用处**。

| # | dsw 参考（file:line） | 值/语义 | 本仓等价 | 落地建议 |
|---|---|---|---|---|
| 1 | `ui-theme/src/styles/design-platform.css:5-77`（`--dsw-static-*` 静态色板，:26 deepseek-450 `rgb(86,134,254)`） | 品牌蓝/中性灰阶 | `--primary:#07c160`（index.css:14）是微信绿，非蓝 | 建议新增 `@theme { --color-brand-blue: #4d93f8; }`（近 static-blue-450 design-platform.css:13） |
| 2 | `ui-sidebar/src/client/SidebarRoot.module.css:16`（`--dsw-specific-sidebar-fill`） | 侧栏底色（独立于对话区） | `--sidebar`（index.css:31/76） | 复用 `bg-sidebar`；如需更暗一档再加 `--color-sidebar-sunken` |
| 3 | `SidebarRoot.module.css:17`（`--dsw-alias-label-primary`） | 主文本 | `--foreground`（index.css:9/54） | 直接映射 `text-foreground` |
| 4 | `SidebarRoot.module.css:193`（iconButton `--dsw-alias-label-secondary`） | 次级图标/文本 | `--muted-foreground`（index.css:19/64） | `text-muted-foreground` |
| 5 | `ui-workspace/src/client/rows/Rows.module.css:131`（`.slot` `--dsw-alias-label-tertiary`）；`WorkspaceBrowser.module.css:51`（sectionHeader） | 三级弱文本 | 无三级 | 建议新增 `--color-faint: color-mix(in oklch, var(--muted-foreground) 75%, transparent)` |
| 6 | `Rows.module.css:359`（chevron `--dsw-alias-label-caption`，注释 #ADB2B8） | caption 灰 | `--muted-foreground` | 同 #5，可共用 `text-faint` |
| 7 | `SidebarRoot.module.css:196-198`（hover `--dsw-alias-interactive-bg-hover`）；`Rows.module.css:13-16` | 行 hover 填充 | `--accent`（index.css:20）或 `--wx-hover:#f0f0f0`（index.css:46） | 统一用 `bg-accent`；wx 面内用 `bg-wx-hover` |
| 8 | `SidebarRoot.module.css:330-334`（`.panelActive` `--dsw-alias-interactive-bg-active` + font-weight 500） | 选中态填充 | `--accent` | 选中= `bg-accent font-medium`（ref 选中/hover 同色，靠字重区分） |
| 9 | `SidebarRoot.module.css:260`（newSession 边框 `--dsw-alias-border-l3`，0.5px）；`ui-conversation/src/client/skeleton/ConversationRoot.module.css:47`（header 下划线 0.5px） | hairline 边框 | `--border`（index.css:23） | `border-[0.5px] border-border`（ref 全线 0.5px，非 1px） |
| 10 | `ui-workspace/src/client/rows/WorkspaceBrowser.module.css:160`（搜索框展开 `--dsw-alias-border-l4`）；`Rows.module.css:184`（renameInput） | 输入控件描边 | `--input`（index.css:24） | `border-input` |
| 11 | `ui-conversation/src/client/skeleton/ConversationRoot.module.css:200-204`（激活 tab `--dsw-alias-state-business-primary` 蓝）；`Rows.module.css:143-145`（folderActive） | 语义蓝（选中/活态） | `--info`（index.css:41 oklch 240°） | 复用 `text-info`，或与 #1 合并为 brand-blue |
| 12 | `WorkspaceBrowser.module.css:480-484`（`--dsw-alias-state-error-primary`） | 危险色 | `--destructive`（index.css:22/67） | `text-destructive` |
| 13 | `ui-theme/src/styles/base.css:7-8`（`--dsw-font-family` 栈） | UI 字体 | tailwind 默认 sans | 建议新增 `@theme { --font-sans: -apple-system, …, 'PingFang SC', 'Microsoft YaHei', sans-serif; }` |
| 14 | `base.css:9-10`（`--ds-font-family-code`，刻意不带裸 `monospace` 尾——防 Win CJK 落 SimSun） | 代码字体 | 无显式 token | 建议新增 `--font-mono: 'SF Mono','JetBrains Mono',…,'Microsoft YaHei'` |
| 15 | `base.css:11-14`（`--ds-ease-in-out cubic-bezier(0.4,0,0.2,1)`；duration 0.2/0.1/0.3s） | 全站动效曲线/时长 | tw-animate-css 无统一曲线 | 建议新增 `@theme { --ease-standard: cubic-bezier(0.4,0,0.2,1); }`，动效取 150/180/300ms 三档（见 #16） |
| 16 | `SidebarRoot.module.css:45-50`（150ms fade）、:53-55（200ms wide-in）、:66-86（rail-in 150ms translateX(49px)）；`SidebarRoot.tsx:31`（`COLLAPSE_SETTLE_MS=150`）、:39（`SCROLLBAR_LINGER_MS=2000`） | 折叠两段动画参数 | 无 | 建议新增 CSS 变量组 `--dsh-collapse-ms:150; --dsh-expand-ms:200; --dsh-slide-ms:300`（300ms 见 WorkspaceBrowser.tsx:37） |
| 17 | `ui-theme/src/styles/scrollbar.css:24,66-68`（8px bar）、:18-19（thumb l1/l2 别名对）、`SidebarRoot.module.css:40-43`（quietBars 置 transparent 保 gutter）；`WorkspaceBrowser.module.css:3-4,350`（2px 边距 + `scrollbar-gutter:stable`）；`ConversationRoot.module.css:349,355` | 主题化细滚动条 | `.scroll-slim` 8px+thin（index.css:154-189）已等价 | 复用 `.scroll-slim`；建议补 `scrollbar-gutter:stable` 约定与 2px 右边距（`mr-[2px]`） |
| 18 | `ConversationRoot.module.css:28-31`（`--dsh-chat-content-width: clamp(680px, column*0.64, 920px)`）、:32（`--dsh-composer-card-max-width: W+32px`）、:33-34（clearance 16px / dock-inset 8px） | 聊天内容宽度轴 | 无 | 建议组件根声明同名 CSS 变量（tailwind 侧 `max-w-[var(--dsh-chat-content-width)]`），JSX 用 ResizeObserver 发布列宽（ConversationRoot.tsx:163-205 模式） |
| 19 | `ui-chat/src/client/chat/ChatView.module.css:39-41`（transcript `max-width: var(--dsh-chat-content-width); margin:0 auto`）、:15（`padding:16px calc(clearance+16px)`）、:51/63（flow-gap 16px→压缩 8px） | 消息列几何 | 无 | `mx-auto w-full max-w-[var(--dsh-chat-content-width)]` + `space-y-2` |
| 20 | `ui-chat/src/client/chat/MessageItem.module.css:28-34`（气泡 `--dsw-specific-bubble`、radius 22px、pad 10/16、字号 14px）、:45 等（secondary 13px） | 消息气泡 | `--wx-bubble-*`（index.css:47-48,90-91）是微信风 | agent 聊天建议新增 `--color-bubble: var(--card)`；radius 用 `rounded-[22px]`，正文 `text-sm`（14px）/辅文 `text-[13px]` |
| 21 | `ui-conversation/src/client/skeleton/InputBar.module.css:53-63`（输入卡 radius 22px、`--dsw-specific-input-major`、`--dsw-elevation-soft`、字号 14/行高 24+delta）、:167-171（textarea min-height 36px）、`ConversationRoot.module.css:316`（草稿上限 336px=14 行） | 输入卡 | PromptComposer 样式未读（预算所限，见 §3 尾注） | 建议新增 `--color-input-major`（亮=#fff/暗=中性-850）+ `shadow-sm` + `rounded-[22px]` + `max-h-[336px]` |
| 22 | `ui-conversation/src/client/skeleton/HeroShell.module.css:35-71`（hero 标题 26px wt500 + preview 徽标 bg business-tertiary 12px）、`EmptyHero.tsx:49-59`（workspace chip：folder 图标+label+chevron 12px） | 空态 hero 字阶 | 无 | 标题 `text-[26px] font-medium`，徽标 `text-xs bg-info/15 text-foreground rounded-md px-[7px]` |
| 23 | `ui-workspace/src/client/rows/Rows.module.css:94,105,27`（行高：组头 34px / 会话 32px / 搜索行 min 48px）、:6（radius 8px）、`WorkspaceBrowser.module.css:353-357`（行间 2px）、:376-378（组间 4px） | 树行几何 | 无 | 建议常量类：`h-[34px]`/`h-8`/`min-h-12 rounded-lg`，行距 `space-y-0.5`、组距 `space-y-1` |
| 24 | `SidebarRoot.module.css:25-30`（56px rail、10px 侧距、36px 控件、顶部 18px）、:250-269（newSession 38px 高 radius 12px 0.5px 边框） | 侧栏壳几何 | 无（现侧栏是 Card，session-sidebar.tsx:83） | rail 状态可选做；new-session 按钮先按 `h-[38px] rounded-xl border-[0.5px]` 复刻 |

≥15 条达标（24 条）。**建议新增 token 汇总**（写入 `index.css` `:root/.dark` + `@theme inline`）：
`--color-brand-blue`(#1)、`--color-faint`(#5)、`--font-sans/--font-mono`(#13/14)、
`--ease-standard`(#15)、折叠动效时长组(#16)、聊天宽度轴组(#18)、`--color-bubble`(#20)、
`--color-input-major`(#21)。

---

## 2. 工作区→会话树规格

### 2.1 组件结构树

```
SidebarRoot（壳，SidebarRoot.tsx:166-276）
├─ logoRow：品牌（点击=新建会话）+ 折叠开关（SidebarRoot.tsx:180-227）
├─ newSession：38px「新会话」按钮（SidebarRoot.tsx:229-240）
├─ panelList：全局面板行（本仓无此概念，砍）
├─ regionArea ← WorkspaceBrowser（侧栏中段，SidebarRoot.tsx:258-265）
│  ├─ sectionHeader（WorkspaceBrowser.tsx:1125-1234）
│  │  ├─ sectionLabel「工作区/会话」标题（:1126-1130，搜索展开时收起让位）
│  │  ├─ 搜索框（内联，圆形→展开为 10px 圆角输入，:1131-1186）
│  │  ├─ ViewOptionsMenu：分组(工作区|单列表)×排序(手动|更新)（:166-212）
│  │  └─ 「添加工作区」+ 按钮 → WorkspacePickFlow（:1201-1233；WorkspacePicker.tsx:58-217）
│  └─ listArea：SessionTree | FlatList | SearchResults 三选一（:1256-1327）
│     └─ groupSection × N
│        ├─ ProjectRowItem 组头（Rows.tsx:112-215）
│        ├─ SessionNodeItem × n（Rows.tsx:379-513）
│        └─ sessionOverflowButton「展开 N 个」（WorkspaceBrowser.tsx:599-610）
└─ footArea：设置区（本仓映射为现有设置入口）
```

### 2.2 交互清单（可判定：每条附「验收证据」）

| # | 交互 | 规格（file:line） | 验收方法 |
|---|---|---|---|
| I1 | 分组规则 | 按工作区实体分组，组序=宿主工作区顺序；不属于任何工作区的会话落 Ungrouped 尾组（tree.ts:206-244，`UNGROUPED_KEY=''` tree.ts:21）。本仓映射：**cwd 目录名分组**（§4） | mock 3 个 cwd + 1 个无 cwd 会话 → 4 组且无 cwd 者在最后；单测断言组序 |
| I2 | 组头行 | 34px：folder 图标（开/闭两态）+ 标题；hover 时 folder 换成右向箭头，箭头随展开旋转 90°（Rows.tsx:149-154，Rows.module.css:149-160）；含当前会话的组 folder 变蓝（Rows.tsx:127-128，Rows.module.css:143-145） | hover 组头截图对比 icon 交换；选中该组内会话后 folder 色变 info |
| I3 | 折叠/展开 | 点组头切换；状态持久化 `groupExpansion`（WorkspaceBrowser.tsx:300-307）；**含当前会话的组自动展开**（:304-307）；折叠组不渲染子行（tree.ts:317-319） | 折叠→刷新→仍折叠；点其它组内会话→该组自动展开 |
| I4 | 行内限流 | 折叠态每组只显 5 条（`COLLAPSED_SESSION_LIMIT=5`，WorkspaceBrowser.tsx:43-58），blank 占位不计限额；超出显「展开 N 个」按钮，点击临时全开（:599-610） | 造 7 条会话断言可见 5+按钮；点按钮全显 |
| I5 | 行结构 | 32px 会话行：16px 状态槽 + 标题 + 相对时间 + ellipsis 菜单；hover 时**时间隐去、菜单浮现**，菜单开着钉住 hover 态（Rows.tsx:462-500，Rows.module.css:231-257） | hover 行截图：time 消失、⋯ 出现；打开菜单移开指针行仍高亮 |
| I6 | 状态点优先级 | pending(审批/计划/提问，warning)>运行中(ongoing)>完成未读(done 绿点)>idle 隐藏（Rows.tsx:231-268，:405 `showStatus`）；本仓映射：`promptPendingBySession`=pending、transcript 活跃=running | 权限待批会话行显黄点；读档后黄点消失 |
| I7 | 行菜单 | 重命名/分叉(fork)/归档(archive)，归档免确认（Rows.tsx:416-421，WorkspaceBrowser.tsx:1078-1086）；本仓映射：重命名保留、fork/归档按 §4 裁剪，关闭会话沿用现有确认框 | 菜单出现三项（裁剪后按实际）；归档后行即时消失 |
| I8 | 搜索 | 头部圆形按钮展开为内联输入（180ms，WorkspaceBrowser.module.css:85-127,148-164）；本地过滤标题+工作区名（tree.ts:392-405），防抖 250ms 后追加分面内容搜索（WorkspaceBrowser.tsx:39,980-1015——本仓砍远端）；Esc 清空收起（:1165-1169）；查询**折叠态存活**（:912-914） | 输入关键字→组树切换为扁平结果列表（min-h-48 双行：标题+工作名气条，Rows.module.css:22-90）；Esc 还原树 |
| I9 | 当前会话选中态 | 行 `aria-selected` + hover 同色填充（Rows.tsx:427，Rows.module.css:18-20）；打开面板时列表选中态让位为空（WorkspaceBrowser.tsx:288 `panelActive`——本仓无对应，砍） | 点行→`bg-accent`；再点另一行→旧行褪色 |
| I10 | 空态 | 列表空：`empty.none` 文案块 pad 16/12 三级灰（WorkspaceBrowser.tsx:460-462，WorkspaceBrowser.module.css:450-454）；搜索无结果单独文案（:820-822） | 清空 store 后显空态文案而非白板 |
| I11 | 底部渐隐 | 列表底部 24px 透明→侧栏色渐变遮罩（WorkspaceBrowser.module.css:312-320），滚动到底时末行在遮罩上方（:347-349 pad-bottom 16px） | 长列表截图底部有渐隐；滚到底末行完整可读 |
| I12 | 滚动条随指针 | 指针不在侧栏时 thumb 置 transparent（gutter 保留不回流），离开后 linger 2000ms 才隐藏（SidebarRoot.tsx:121-162，SidebarRoot.module.css:40-43） | 指针移出侧栏 2s 后滚动条消失；移入即现且无行抖动 |
| I13 | 侧栏折叠动画 | 两段式：内容冻结原宽原地 150ms 淡出 → 轨道滑动裁切 → 150ms rail-in（图标自 49px 位移入场）；展开反向 200ms wide-in；`prefers-reduced-motion` 全禁（SidebarRoot.module.css:45-86,418-429；SidebarRoot.tsx:31,100-119） | 录屏无中途 reflow 跳变；系统减动效开启时动画消失 |
| I14 | 搜索/添加的 rail 形态 | 折叠态区头只剩 36px 搜索+添加圆钮；点搜索=展开侧栏+300ms 后聚焦输入框（WorkspaceBrowser.tsx:946-956,1237-1252） | 折叠态点放大镜→侧栏展开且焦点落在输入框 |
| I15 | 排序 | `manual`（宿主顺序/拖拽序，WorkspaceBrowser.tsx:99-163）与 `updated`（新更新置顶，首次切换全量按新近重排，之后仅一次性提升有新动静的会话 :139-151）；id 兜底 tiebreak（tree.ts:133-137）。本仓建议只做 updated（§4） | 切「按更新」→ 有新消息的会话跳到组顶且下次刷新不重排 |
| I16 | 拖拽排序 | 会话拖拽出上/下半插入线（Rows.module.css:259-299），组内提交+宿主持久化（WorkspaceBrowser.tsx:375-427）；工作区行可拖换序（:428-447）。**本仓建议砍**（§4） | —（若做：拖拽时行间出现蓝色 2px 插入线） |
| I17 | HoverCard | 组头卡：标题+完整路径(~缩写)+创建时间；会话卡：标题+相对时间+状态行；菜单打开时抑制（Rows.tsx:199-214,503-512）。本仓可选（title 属性兜底即可） | hover 500ms 后出卡；右键菜单开着不出卡 |

### 2.3 与任务书要求的差异点

- 「当前工作区置顶」：参考实现**不做置顶**，等价诉求由两条满足——当前组自动展开（I3）+
  组头 folder 蓝色高亮（I2）。建议本仓跟随参考（置顶会破坏稳定组序的心智模型）。
- 双视图（工作区分组 / 单列表）由 ViewOptionsMenu 提供（I15 上游，WorkspaceBrowser.tsx:166-212），
  本仓第一版建议只做分组视图，菜单砍。

---

## 3. 聊天区视觉差距清单（参考实现 vs 本仓现状）

| 部位 | 参考实现（file:line） | 本仓现状（file:line） | 差距动作 |
|---|---|---|---|
| 会话头 | `ConversationRoot.module.css:42-48`：76px 高（=侧栏 tab 条对齐）、pad 10/28/0/20、0.5px 底边线；面包屑 `crumbs`（ConversationSession.tsx:72-133）：层级路径「/」分隔、sep 14px caption 灰（ConversationRoot.module.css:87-92）、当前段 wt500 主色、段 hover 圆角填充；右侧 `headerActions`/`headerUtilities` 槽（:124-141） | `agent-conversation.tsx:179-185`：h-12 一行，Bot 图标+标题+wsUrl 弱文本，`border-b`（1px） | ① 头高提为 76px（或先 56px 折中）+ `border-b-[0.5px]`；② 标题右侧 wsUrl 换成工作区路径面包屑（cwd basename / 完整路径两段）；③ 预留右侧动作槽 |
| 空态（hero） | 三阶段根 `data-phase=hero/active/settling`（ConversationRoot.tsx:272-273,355,372-373）；hero=列内垂直居中栈：HeroShell（鱼 logo 34px+标题 26px+preview 徽标，HeroShell.module.css:35-71）→ 工作区 chip 行（folder+「选择工作区」+chevron，EmptyHero.tsx:49-59，行距 8px ConversationRoot.module.css:440-463）→ hero 变体输入卡；同栏仍可滚动 | `agent-conversation.tsx:187-193`：无会话时仅居中一行提示文案；`chat-page.tsx:223-229` ChatEmptyState 是计数卡 | 复刻 hero：居中「标题+工作区选择 chip+输入框」三行栈（本仓工作区=已有 cwd 分组的选择器）；settling 防闪（`visibility:hidden` 占位，ConversationRoot.module.css:472-476）可选 |
| 输入区 | 输入卡：22px 圆角、`--dsw-specific-input-major` 底、soft 阴影、0.5px stroke（InputBar.module.css:53-63）；textarea min-h 36、caret 蓝（:167-181）；hero/docked 双变体同宽（ConversationRoot.module.css:440-451）；栈：dock 卡（todo/queue）6px 间距叠在输入卡上方（:292-302）；粘性座位+顶部 36px 渐隐遮罩（:369-386）；草稿上限 336px | `agent-conversation.tsx:196-197`：Transcript + PromptComposer 直接纵排；PromptComposer 内部样式未读（预算所限） | 输入卡圆角/阴影/遮罩按 #21 token 落地；composer 未读项**遗留**：需一轮 prompt-composer.tsx 对照再定差距明细（已在 §4 建议列为后续 T 任务） |
| 消息列 | 内容列居中 `clamp(680,64%,920)`（#18/#19）；行 gap 8px（ChatView.module.css:63）；气泡 22px 圆角 pad 10/16（#20）；回到底部按钮 z-8 上浮（ChatView.module.css:172-204） | transcript 组件未读；页面无内容宽度轴，通栏铺满 | 建内容宽度变量+居中列；气泡与间距按 §1 #19/20 落地 |
| 滚动域 | `data-conversation-scroll`：`scrollbar-gutter:stable` + 右 2px 偏移（ConversationRoot.module.css:341-362）；主题化 8px bar（#17） | `.scroll-slim` 已 8px/thin/hover 提亮（index.css:154-189），无 gutter 约定 | 滚动容器统一 `scroll-slim`+`scrollbar-gutter:stable`+`mr-[2px]` |
| 纵向宽度拖拽 | 左右 40px col-resize 手柄+光标跟随光带，偏好 localStorage 持久（ConversationRoot.tsx:16-44,116-235；ConversationRoot.module.css:226-290） | 无 | **建议砍**（§4）：交互成本高、收益边际 |

---

## 4. 数据映射与实现建议

### 4.1 数据面

- 本仓数据源：`acp-store.ts:51` `sessions: SessionSummary[]`（来自 ACP `session/list`），
  `activeSessionId`(:52) 为当前选中；无 workspace 实体、无归档/分叉/子代理概念。
- `SessionSummary.cwd`：任务书口径「session/list 返回 cwd」；`apps/gui/src/acp/protocol.ts`
  本次未读（预算），**实施前须核对 `cwd` 字段名与空值语义**（空/缺失 → Ungrouped 组）。
- 参考实现的分组权威是宿主 workspace 实体（tree.ts:33-35 membership 反查）；本仓零后端，
  改为**cwd 派生分组**：组键=cwd 规范化字符串，组名=basename（workspaceLabel，tree.ts:127-131 同规则）。

### 4.2 建议挂在 acp-store 的纯函数形态（可原样落 `apps/gui/src/acp/workspace-model.ts`）

```ts
// 全部纯函数；store 侧只存 UI 态，不做派生
interface WorkspaceGroupNode {
  key: string;            // 规范化 cwd；无 cwd → ""（Ungrouped，恒排最后）
  label: string;          // basename(cwd)；Ungrouped 用 i18n 文案在渲染层兜底
  cwd: string | null;
  sessions: SessionSummary[];   // 已按 updated 降序、sessionId tiebreak
  containsCurrent: boolean;
}
function workspaceLabelOf(cwd: string | null | undefined): string;
function groupSessionsByWorkspace(
  sessions: readonly SessionSummary[],
  currentSessionId: string | null,
): WorkspaceGroupNode[];        // 组序：cwd 首次出现序（列表序稳定），Ungrouped 恒末位
function collapseGroupRows(
  sessions: readonly WorkspaceGroupNode["sessions"],
  opts: { limit: number },     // limit=5，I4
): { rows: typeof sessions; hiddenCount: number };
function filterSessionsByQuery(
  sessions: readonly SessionSummary[],
  query: string,               // trim+lowercase，匹配 title 与所属组 label
): SessionSummary[];            // 本地过滤，无远端搜索
```

zustand 增量（不进 AcpConsoleState 主表，独立 slice 防污染）：

```ts
interface WorkspaceUiSlice {
  expandedGroupKeys: string[];          // 持久化可选（参考为宿主持久化，I3）
  transientExpanded: string[];          // I4 的「展开 N 个」临时态
  workspaceQuery: string;               // I8：折叠侧栏不清空
  toggleGroup(key: string): void;
}
```

派生用 selector（`useAcpStore(s => groupSessionsByWorkspace(s.sessions, s.activeSessionId))`
+ memo）在组件层完成，store 不冗余存树。

### 4.3 建议砍掉的过度设计（第一版不做）

| 砍 | 理由 |
|---|---|
| 拖拽排序（会话/工作区两级 + 宿主持久化 + 折叠态锚点解析，I16） | 本仓无持久化宿主；成本/收益最差的一块（WorkspaceBrowser.tsx:375-447 近 300 行） |
| workspace 重命名/删除/添加对话框 + 目录选择流（WorkspacePicker.tsx 全部） | 本仓工作区是 cwd 派生虚实体，不可增删改名 |
| `groupBy: flat` 单列表视图与 ViewOptionsMenu | 单列表即现状，保留分组视图即可 |
| 内容搜索（远端 `session.search` + 防抖合并 + hasMore 分页，tree.ts:365-440） | 无后端；I8 只保留本地过滤 |
| fork / 归档 / 子代理运行计数 / schedule 指示（Rows.tsx:283-296,417-421；tree.ts:269-271） | ACP 面无对应能力；状态点保留 pending/running/done 三态（I6） |
| 纵向宽度拖拽手柄（§3 末行） | 边际收益低 |
| 侧栏 56px rail 折叠态（I13/I14） | 本仓侧栏在 /chat 双栏布局内，非全局壳；可后置为独立任务 |
| HoverCard（I17） | 用原生 `title` 属性替代，留后续 |

**保留并复刻**：cwd 分组树（I1-I4）、行 hover 交换（I5）、状态点（I6）、本地搜索内联展开（I8）、
当前组自动展开+folder 高亮（I2/I3）、空态 hero（§3）、底部渐隐（I11）、`.scroll-slim` 对齐（#17）。

### 4.4 遗留项（本调研未覆盖，建议追加小任务）

1. `apps/gui/src/acp/protocol.ts` 核对 `SessionSummary.cwd` 字段与 `session/list` 实际返回。
2. `apps/gui/src/acp/components/prompt-composer.tsx`、`transcript.tsx` 未读 → §3 输入区/消息列
   差距只到页面层，组件级差距需补一轮对照。
