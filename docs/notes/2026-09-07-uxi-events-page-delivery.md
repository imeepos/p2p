# UX-I 事件页可读性 · 交付报告（feat/uxi-events-page）

> 派单：docs/notes/2026-09-07-ux-polish-dispatch.md 波 2（F16/F20）
> 状态：完成，已 push origin。main 未动，待机械验收后统一合并。

## 分支
- tip：见 git log（合并 origin/main 后的四门禁终验全绿）
- 提交序列：4245391(i18n uxiEvents 键块，独立 append-only) → 5ead1d3(feat 事件行展开反馈 + 筛选分组计数) → af222ff(反向同步 origin/main 7ca36a4，零冲突)
- 变更：8 文件 +410/−44 + i18n 2 文件 +32；全部文件 ≤150 行；路由/menu 未动
- 文件清单：
  - apps/gui/src/views/network/events/event-filter-groups.ts（新增，五域分组 + countEventsByType）
  - apps/gui/src/views/network/events/events-filter-bar.tsx（重写，分组 chip+计数+清除筛选）
  - apps/gui/src/views/network/events/event-row.tsx（重排，data-state 容器 + 行主体/行尾详情双入口 + 展开高亮）
  - apps/gui/src/views/network/events/use-events-controller.ts（counts memo 接通）
  - apps/gui/src/views/network/events/events-view.tsx（传 counts/onResetFilters）
  - apps/gui/src/i18n/locales/{zh-CN,en-US}.ts（末尾 uxiEvents 键块，zh=en=1048）
  - 新增测试 3 件：event-filter-groups.test.ts / events-filter-bar.test.tsx / event-row.test.tsx（17 用例）

## 四项门禁（合并 origin/main 后终验）
- vitest：173 文件 / 1021 用例全绿（基线 170/1008；+3 文件 +13 用例）
  - 注：并行会话同机负载下 workers 偶发启动超时/时序用例假红（前两轮全量各挂 1/4 个基建性失败，失败文件隔离复跑秒级全过），按 self-evolving 已知处置以 --no-file-parallelism 稳定化重跑，语义不变
- eslint：0 错 0 警
- tsc -b：0 错
- check:i18n：PASS（zh=1048 en=1048，键集合一致）

## F16 事件行展开反馈（页面实测，VITE_MOCK_IPC=1，port 5185 隔离 cacheDir）
操作路径：mock dev 启动（auto-start 节点）→ #/network/events → 事件列表行
- 行主体点击：data-state closed→open、aria-expanded true、「原始负载」区渲染（payload 493 字节 JSON 可见）、展开态整行高亮（getComputedStyle 实测底色 oklab(0.97 0 0 / 0.5) = bg-muted/50）+ 主色内嵌条
- 行尾显式「详情」按钮：同样可展开，aria-expanded 同步
- 收起→再展开：data-state 回 closed、负载消失；再开负载内容一致（samePayload true）——受控 Set 驱动，状态一致
- 证据：/tmp/uxi-3-expanded.png（展开高亮+详情），DOM 断言 JSON 见走查输出
- 单测：event-row.test.tsx 覆盖收起态/行主体回调/详情按钮展开/收起再展开一致性

## F20 筛选 chip 分域分组 + 计数 + 清除筛选（页面实测）
操作路径：#/network/events（节点运行、ticker 出事件）→ 观察分组 → window.__MOCK_CHAT__/__MOCK_GROUP__/__MOCK_GROUP_INVITE__ 注入聊天/群类 → 操作 chip 与清除筛选
- 分组渲染：连接（发现/连接/断开/逐跳）、消息（聊天消息/消息状态/好友邀请）、群组（群消息/群送达状态/群状态变更/入群邀请）、安全（监听失败/拨号失败/违规/错误）、节点（启动/停止）——17 chip 全部归组
- 计数正确性：ticker 阶段「发现4/连接2/启动1」；注入后「聊天消息2/消息状态1/群消息1/入群邀请1」增量与注入一一对应（好友邀请卡随入群邀请同步产生一条 chat_message，计数自洽）
- 空数据可用：停止节点→清空缓冲（AlertDialog 确认走通）→ 17 chip 全部「，0 条」可见、「暂无事件」空态、chip 可正常切换（aria-pressed 翻转）
- 0 计数可见：安全组 4 chip 全程 0（mock 无错误事件注入路径），证明 0 计数常态可见
- 清除筛选：关 2 chip + 开「仅错误」→ 清除筛选可用 → 点击 → 17 chip aria-pressed 全 true、开关复位、按钮回到 disabled ✓
- 证据：/tmp/uxi-0-empty.png（空态全 0）、/tmp/uxi-1-groups.png（分组+ticker 计数）、/tmp/uxi-2-counts.png（注入后计数）、/tmp/uxi-4-after-clear.png（清除后恢复）
- 单测：event-filter-groups.test.ts（五组并集=ALL_EVENT_TYPES 机械护栏、计数含空缓冲全 0）；events-filter-bar.test.tsx（分组渲染/空数据 0 计数可切换/计数随缓冲更新/清除按钮禁用态与回调）

## 走查工程备注（喂回 skill）
- Radix AlertDialog 确认按钮在弹窗刚挂载的瞬间吃合成 click 会无效果（原生 trusted click 同样偶发），稳定做法：弹窗出现后 settle ~700ms 再取坐标点击； chip/行等 root 内普通按钮 .click() 即可
- mock dev 有节点 auto-start 且手动停止需等 auto-start 落定后点「停止节点」（manualStopRequested 守卫才生效），「空缓冲」须走 停止→清空→确认 全 UI 路径
- 本会话专用：gui-agent 副本 DEBUG_PORT 9223→9241、vite wrapper 隔离 cacheDir 端口 5185，收尾均已清理

## 备注
- 审计原文 docs/notes/ux-audit-20260907.md F16/F20 与派单一致，无口径出入
- 安全类事件（监听失败/拨号失败/违规/错误）mock 无产生路径，非零计数正确性由单测覆盖（countEventsByType 用例含 node_error/listen_failed）
