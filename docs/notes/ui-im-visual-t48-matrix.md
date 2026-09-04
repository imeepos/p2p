# IM-T48 聊天域终态视觉走查矩阵

> 范围：聊天域（components/chat、views/chat、stores/chat-store、i18n chat 段）。五页与 views/shared 只读未动。
> 方法：无视觉输入会话的三件套交付——类名/DOM 断言测试（vitest/jsdom）+ CDP 几何与样式实测（零依赖驱动，
> 脚本运行于 /tmp 不入库）+ 截图留档（docs/notes/ui-im-visual-t48/）。静态代码走查与 token 对比度离线计算
> （oklch→linear sRGB→WCAG luminance，透明色在 gamma sRGB 空间合成；锚点校验：白/黑=21.0、#767676/白=4.54）。
> 环境：worktree feat/im-t48-final-audit，VITE_MOCK_IPC=1，vite dev 端口 5213，headless Chrome 1440×900。
> 定级：高=可用性 / 中=观感 / 低=打磨。处置：高中 CSS 级直接修复（独立 fix 提交），结构性报协调裁决，低留档不修。

## 一、走查矩阵（需求维度 × 结果 × 截图 × 定级）

### a. 聊天页全组件

| 组件 | 走查项 | 结果 | 定级/处置 | 证据 |
| --- | --- | --- | --- | --- |
| 好友列表条目 | 在线状态点/昵称/摘要/时间四行布局，昵称 truncate，摘要 truncate，倒序排列 | PASS | - | 20-23 格截图；chat-view.test 排序断言 |
| 好友列表 loading 态 | 误用 common.state.unknown 显示"未知" | 修复 | 中 | f186a76；新用例断言"正在加载好友…" |
| 好友列表 error 态 | 仅红字无重试入口，偏离 B6 全局规范 | 修复 | 中 | f186a76；红字+刷新按钮+重试成功用例 |
| 会话空态（选中好友无消息） | 纯白面板；chat.noMessages 死词条 | 修复 | 中 | b03083f；EmptyState 渲染用例；02-empty-conversation.png |
| 消息气泡 | me 靠右/them 靠左，max-w-75%，时间+状态角标，文本 whitespace-pre-wrap break-words | PASS | - | 06-message-2000.png；T47 矩阵 15 格 |
| 状态角标 | failed 红字在 me 气泡上对比度不达 AA | 修复 | 中 | 2cd0fca；对比度表见 §f |
| 回复引用块 | tone 双底色、点击跳转高亮、缺失占位不白屏 | PASS | - | 03-quote-block.png；chat-reply.test 6 用例 |
| 引用块超长摘要 | 无截断约束，超长文件名逐行折行/不折行横向溢出 | 修复 | 中 | 9adf6c9；§e 几何实测 |
| 媒体五类渲染 | 图片/音频/视频内联 + 文件信息卡兜底；内联媒体无宽度上限 | 修复 | 中 | 45ce383；img/video max-w-full 断言 |
| 输入条 | 多行 Textarea、表情面板 32 格、附件按钮、Enter 发送/Shift+Enter 换行、超长红边+提示 | PASS | - | 10-add-dialog.png 顶部；chat-composer.test |
| 添加好友对话框 | sm:max-w-lg、Label htmlFor 对应、字段错误/后端原文双通道、双按钮 footer | PASS | - | 10-add-dialog.png；chat-friend-add.test |
| 移除确认框 | sm:max-w-md、说明+历史须知双行、默认焦点取消、destructive 确认 | PASS | - | 11-remove-dialog.png；chat-friend-remove.test |
| 取消发送入口 | pending 占位附"取消发送"，点击移除占位且摘要回滚 | PASS（本轮修复后） | - | 31/32 截图对；cancelDemo：取消后摘要指向真实消息 |

### b. 双主题（dark/light）

机械信号四格全绿（每格全量探针：consoleErrors=0、exceptions=0、overflowX=0、htmlDark 与目标一致、
rawKeys 仅剩测试夹具文件名 finalv2.zip、应用内错误缓冲 0）：

| 格 | light zh | dark zh | light en | dark en |
| --- | --- | --- | --- | --- |
| 聊天页（含列表/气泡/引用块/媒体卡/输入条） | PASS | PASS | PASS | PASS |
| 截图 | 20-light-zh.png | 21-dark-zh.png | 22-light-en.png | 23-dark-en.png |

- 主题即时性：设置页外观卡真实点击切换（浅色/深色 chip），htmlDark 立即翻转，dots/statuses computed color
  随主题翻转（离线点 oklch(0.556 0 0)↔oklch(0.708 0 0)，气泡正文 oklch(0.985 0 0)↔oklch(0.205 0 0)）。
- portal 对话框（添加/移除）跟随主题 token（bg-background/border），两主题截图核验。

### c. 双语言（zh-CN/en-US）

| 走查项 | 结果 | 证据 |
| --- | --- | --- |
| 切换即时性 | PASS：语言 chip 点击后立即生效（发送中↔Sending），来回可逆，无残留 | 格 20↔22、21↔23 |
| 按钮换行 | PASS：发送/添加/取消等按钮 whitespace-nowrap（Button 组件内建），zh/en 均单行 | 四格截图 |
| 截断策略 | PASS：昵称/摘要/文件名 truncate；引用块 line-clamp-2；时间戳格式随 locale | §e |
| 对话框宽度 | PASS：sm:max-w-lg / sm:max-w-md 两语言下无溢出 overflowX=0 | 四格机械信号 |
| i18n 完整性 | PASS：check:i18n 434=434；rawKeys 探针仅剩夹具文件名（非 UI 残留） | 机械信号 |

### d. 三态一致性

| 区域 | 加载态 | 空态 | 错误态 |
| --- | --- | --- | --- |
| 好友列表 | 修复：显示"正在加载好友…"（原误显"未知"） | PASS：EmptyState（MessageCircle 圆底图标+引导+主操作居中） | 修复：红字原文+刷新按钮（对齐 B6 形态） |
| 消息面板 | PASS：分页加载顶部提示 loadingHistory | 修复：EmptyState"暂无消息"（原纯空白，noMessages 死词条） | 走查缺口：见 §三 S2 |
| 发送路径 | PASS：pending 角标+取消入口 | - | PASS：toast 发送失败+原因（mock 无失败路径，由组件测试覆盖） |

三态组件均复用 EmptyState/LoadFailedNotice 形态规范（图标圆底 size-12、居中、max-w 限宽）。

### e. 极端内容

| 内容 | 结果 | 定级/处置 | 证据 |
| --- | --- | --- | --- |
| 2000 字符消息 | PASS：恰好渲染，whitespace-pre-wrap 换行，无布局爆炸（气泡 295px 宽内折行） | - | 06-message-2000.png；T47 矩阵 2000 字符用例 |
| 超长文件名（引用块） | 修复后 PASS：line-clamp-2 两行截断，CDP 实测短摘要 24px（1 行）vs 长文件名 40px（2 行封顶） | 中（已修） | 9adf6c9；05-quote-longname.png |
| 超长文件名（媒体卡） | PASS：truncate 单行省略 | - | 媒体信息卡 DOM 断言 |
| 超长昵称（64 字符） | PASS：truncate 单行省略，不挤掉状态点与时间 | - | 07-long-nickname.png |
| 超长 PeerId | PASS：列表缩略 slice(0,12)、头部完整 44 字符 font-mono 宽度充裕（1024 视口实测不溢出） | - | 四格截图头部区域 |
| 回复摘要文本 80 字符截断 | PASS：replySummaryOf 截断加省略号 | - | T46B REPLY_ROWS 断言 |

### f. 状态色彩语义（token 离线 WCAG 计算 + CDP computed style 实证）

| 前景/背景 | 亮色 | 暗色 | 判定 |
| --- | --- | --- | --- |
| me 气泡正文 primary-foreground / primary | 17.16 | 14.22 | AA |
| me 气泡时间/状态 @80% | 11.22 | 8.20 | AA |
| failed 角标 原状 text-destructive / primary | 3.76 | 2.30 | 不达 AA（小字 4.5）→ 修复 |
| failed 角标 修复后 text-red-300 / dark:text-red-700 | 9.33 | 5.10 | AA |
| 在线点 bg-success / 行背景 | 3.30 | 7.93 | 过 1.4.11 图形 3:1 |
| 离线点 原状 muted-foreground/40 | 1.69 | 2.11 | 不达 3:1 → 修复 |
| 离线点 修复后 全量 muted-foreground | 4.73 | 7.63 | 过 3:1 |
| them 气泡正文 / bg-muted | 16.42 | 14.48 | AA |

CDP computed style 实证：四格中角标/状态点颜色与上表 token 一一对应（含修复后取值）。

## 二、修复清单（均为独立可 revert 提交，fix 带回归）

| 提交 | 内容 | 定级 | DOM 断言证据 |
| --- | --- | --- | --- |
| a2189c7 | 裁决项：占位移除统一回滚会话摘要（retractPending；dropPending 失败回滚 + cancelPending 同机理一并接入；期间新事件摘要不覆盖） | 高（裁决转本单） | stores/chat-store.test 4 用例（修前 3 红）：媒体失败回退/无历史清空/在途新消息不覆盖/取消回退 |
| bff4b3a | i18n 登记 chat.friendsLoading（append-only 独立小提交） | - | check:i18n 434=434 |
| f186a76 | 好友列表 loading/error 态对齐全局规范 | 中 | chat-view.test 三态用例（加载中文案/错误+刷新/重试恢复） |
| b03083f | 空会话渲染"暂无消息"空态 | 中 | chat-view.test 空态用例 + render-matrix 回归 |
| 9adf6c9 | 引用块 line-clamp-2 + overflow-wrap:anywhere | 中 | chat-reply.test 类名断言 + CDP 实测 40px 两行封顶 |
| 45ce383 | 内联媒体 img/video max-w-full 防溢出气泡 | 中 | render-matrix 类名断言（修前红/修后绿） |
| 2cd0fca | failed 角标双主题红类保 AA | 中 | render-matrix failed 角标断言（禁 text-destructive 回退） |
| 8ad7cc7 | 离线状态点全量中性灰 | 中 | online-status.test 色彩断言（bg-success/bg-muted-foreground 无透明后缀） |

## 三、报协调裁决（结构性，不擅自大改）

- S1 会话列表 loading 为纯文字，无骨架屏；与五页 LoadingSkeleton 形态不一致。涉及共享组件形态取舍，
  且聊天列表行高与骨架差异属设计裁决，留裁决。定级：低（信息已可读）。
- S2 选中好友后历史加载失败（selectPeer 拒绝）仅 console.error，无界面信号（面板停留空态，与"暂无消息"
  空态不可区分）。属行为级（需错误态状态机与重试接线），超出 CSS 级边界，报裁决。定级：中。
- S3 mock 后端无法产生 delivered/failed 状态与 them 气泡（心跳只随机连接 randomPeerId，好友永不连接；
  chatSend 无失败路径）。真实后端下这三类状态的 UI 走查在 mock dev 环境不可达，本单以 vitest 渲染矩阵
  （T47 15 格 + 本轮补断言）覆盖渲染逻辑。建议后续给 mock 增加脚本化连接/失败注入（行为/测试基建）。

## 四、低 severity 留档（不修）

- L1 pending/sent/delivered 状态文字单色（无色彩分层），仅文字语义区分；对齐 StatusBadge 四档需气泡内
  徽章化设计裁决。
- L2 好友列表条目时间戳仅最后消息有，无消息行不显示占位（信息密度取舍，观感无碍）。
- L3 表情面板 32 emoji 无分组/搜索（需求即"约 20-40 个不做分类页"，符合预期）。
- L4 空态图标（MessageSquare/MessageCircle）语义可再打磨。

## 五、证据留档

- 截图：docs/notes/ui-im-visual-t48/（02 空会话/03 引用块/05 长名引用/06 两千字/07 长昵称/10 添加框/
  11 移除框/20-23 四格矩阵/31 待发送/32 取消后）。
- 机械信号：四格 overflowX=0、consoleErrors=0、exceptions=0、htmlDark 翻转正确、语言切换即时、
  rightRatio=0.816（右栏占比，T45 打回线 >0.6）。
- 测试：apps/gui vitest 聊天域 9 文件 85 用例全绿（含本轮新增 10 用例）；check:i18n 434=434。
- 对比度：§f 全表由 token 值离线计算（方法与锚点见文首），关键值经 CDP computed style 复核一致。
- 走查驱动脚本（/tmp/t48-walk.mjs）与原始报告（/tmp/t48-report.json）不入库，方法要点已录入本文。
