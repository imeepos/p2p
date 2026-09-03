# GUI 布局技巧（p2p-console）

<!--
  本仓前端：React + Tailwind（shadcn 风）+ react-i18next + zustand。
  条目从真实组件提炼，锚点文件即权威实现——动版式前先对锚点；
  版式被第二次实战修正时按「修订 日期」小节追加。
-->

## B1 页面栅格与卡片（设置页系）
- 页面容器：app-layout.tsx 的 grid grid-cols-12 gap-4 p-6；视图内卡片自带 col-span-12 lg:col-span-6 双列铺排，窄屏自然单列。
- 卡片骨架：Card > CardHeader(CardTitle + CardDescription) + CardContent(className="flex flex-col gap-4")；标题用 t("settings.cards.x")，描述用 hint 键。
- 锚点：views/settings/identity-card.tsx、appearance-card.tsx、profile-card.tsx。

## B2 表单行（标签 + 输入 + 提示）
- 行骨架：flex flex-col gap-1.5 > Label(htmlFor) > Input/Textarea；Input 的 id 与 Label htmlFor 对应（测试 getByLabelText 可寻址）。
- 长度约束用 maxLength 硬上限直接拦输入，省掉整层错误态 UI；辅助说明用 text-muted-foreground text-xs 行。
- 占位符是独立 i18n 键（xxxPlaceholder），不复用 label 键。
- 锚点：views/settings/profile-card.tsx。

## B3 独立保存卡片（局部 dirty，不走全页 save-bar）
- 形态：draft state（useState 初值 null）+ current = draft ?? 已保存值；dirty = 逐字段比较 draft 与已保存值；保存成功后 setDraft(null) 让 store 回读值归零脏状态。
- 按钮禁用条件 !dirty || saving；进行中文案切换（保存中…）单独 i18n 键。
- 适用判据：该卡片保存不要求节点重启、与主配置表单无字段耦合——网络配置仍走全页 SettingsSaveBar + 保存并重启。
- 锚点：views/settings/profile-card.tsx。

## B4 头像/图片上传
- 模式：hidden 的 input[type=file]（accept 白名单 image/png,image/jpeg,image/webp）+ ref.current?.click() 触发；onChange 先取 file 再把 event.target.value 复位为空串（同一文件二次选择也要能触发）。
- 预览：img className="size-16 shrink-0 rounded-full object-cover"；无图占位用 CircleUserRoundIcon 包在 bg-muted 圆底 span 里。
- 错误分流：AvatarFileError.code 决定不同 i18n 文案，console.error 留信号 + toastError 用户反馈双通道。
- 锚点：lib/avatar.ts + views/settings/profile-card.tsx。

## B5 侧边栏条目与身份徽标
- 侧栏宽：collapsed w-14 / 展开 w-60；菜单项 h-9 px-3 text-sm font-medium，active 态 bg-sidebar-accent + font-semibold。
- 徽标形态：头像 size-7 shrink-0 rounded-full object-cover + 文本列（min-w-0 flex-1 leading-tight，两行均 truncate，次行 font-mono text-[10px]）；collapsed 只留头像 + Tooltip side="right"。
- 点击落点：Link to="/settings"，aria-label 与 title 用 editProfile 键。
- 锚点：components/layout/identity-badge.tsx、sidebar.tsx。

## B6 加载失败态
- 统一复用 LoadFailedNotice（红字 + AsyncButton 重试），传 messageKey + onRetry；不要每张卡自己写一套。
- 卡内失败态与内容互斥渲染：loadError 且未 loaded ? 失败态 : 内容。
- 锚点：views/shared/load-state.tsx、views/settings/settings-view.tsx 的 LoadingSkeleton。

## B7 图标与按钮状态
- 图标一律 lucide-react 具名 *Icon 导入 + aria-hidden；装饰色用 text-muted-foreground 控。
- 按钮加载态走 AsyncButton（aria-busy + lucide-loader-circle，成功切 lucide-check 由封装给）；普通按钮的 saving 文案切换参考 profile-card。
- 锚点：components/feedback/async-button.tsx、views/settings/save-bar.test.tsx（对类名的断言即约定文档）。

## B8 i18n 与文案纪律
- UI 文案零硬编码（i18n/hardcoded-copy.test.ts 机械扫描），全部 t("键位")；键位 zh-CN/en-US 同步注册且注册类改动独立小提交。
- 动态文案用插值（双花括号占位）；校验错误码统一 ErrorText 渲染 common.validation 前缀键。
- 锚点：i18n/index.ts、views/shared/error-text.tsx。
