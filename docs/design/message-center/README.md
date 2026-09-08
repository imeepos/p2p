# 消息中心设计稿（gpt-image-2，对齐已实现 GUI）

2026-09-08 用 `gpt-image-2` 生成。提示词里的色值/圆角/组件结构/文案全部取自当前实现，
不是自由发挥：主题令牌读自 `apps/gui/src/index.css`（WeChat 桌面风格 WX1），页面结构读自
`apps/gui/src/views/messages/`，文案读自 `apps/gui/src/i18n/locales/zh-CN.ts` 的 `messages.*`。

## 产物

| 文件 | 内容 |
| --- | --- |
| `messages-page-pending.png` | 默认「待处理」视图：只显示未处理消息 |
| `messages-page-history.png` | 「历史消息」视图：已同意/已拒绝 + 状态筛选 chips |

## 与现有实现的对应关系

- 外壳：28px macOS overlay 标题栏 + 左侧 icon rail（`icon-rail.tsx`）+ 底部状态栏。
- 消息中心 = `/messages` 页（`messages-page.tsx`），非下拉菜单：页头标题 + 描述 + 分区列表。
- 卡片结构 = `friend-invite-section.tsx` / `group-invite-section.tsx` 现有行解剖：
  `bg-card ring-1 ring-border hover:ring-ring rounded-lg p-3`，首行 方向徽章/名称/行内
  操作/状态徽章/时间，次行 mono PeerId + CopyButton，三行 备注，失败行 text-destructive。
- 角标 = rail 消息中心铃铛红角标 `--wx-badge #fa5151`，数量 = 两类 in 向 pending 之和
  （F15 同源 selector）。
- 主色 `#07c160`（WeChat 绿，primary 按钮「同意」），描边按钮「拒绝/撤回」。

## 设计增量（相对当前实现，供后续 feature 参考）

1. **类型图标色**：分区头加彩色圆角方块图标 chip——入群邀请绿 `#07c160`（UsersRound）、
   好友邀请蓝（info 令牌，UserRoundPlus），白 glyph，替代当前无色 h2 标题。
2. **分区计数角标**：分区标题旁红 `#fa5151` 计数 pill，与 rail 角标同源。
3. **待处理/历史切换**：页头右侧 macOS 风格 segmented control——默认「待处理 N」只显示
   pending（in 向可操作行 + out 向可撤回行）；「历史消息」显示已同意/已拒绝，内容淡化，
   增加状态筛选 chips（全部/已同意/已拒绝）。
4. **历史态语义**：状态徽章沿用 `StateBadge` 配色（已同意 = text-primary，已拒绝 =
   muted），操作按钮只剩 out 向待处理的「撤回」。

## 再生成

```bash
bash docs/design/message-center/gen.sh \
  docs/design/message-center/payload-page-pending.json \
  docs/design/message-center/messages-page-pending.png
```

`gen.sh` 取 `.env` 第一对 `OPENAI_API_KEY/OPENAI_BASE_URL`（openai.bowong.cc，模型
`gpt-image-2`，1536x1024 high）。改提示词后用 python3 重建对应 payload json 即可。
