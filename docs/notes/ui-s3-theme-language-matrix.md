# W6-S3 主题×语言全矩阵走查

> 范围：7 路由（仪表盘/节点/事件/发现/中继/诊断/设置）× light/dark × zh-CN/en-US = 28 格。
> 方法：仓库内 CDP 零依赖驱动（复用 scripts/gui-agent.mjs 技法，走查脚本运行于 /tmp，
> 未入库）+ 静态代码走查 + 即时性探针。环境：worktree dev server（VITE_MOCK_IPC=1，
> 端口 5199，headless Chrome 1440×900）。

## 一、机械走查矩阵（28 格）

每格全量刷新后静置 1.5s，采集六项机械信号（结果全绿）：

| 信号 | 含义 | 结果 |
| --- | --- | --- |
| consoleErrors | 控制台 error 级输出 | 28/28 为空 |
| exceptions | 未捕获异常 | 28/28 为空 |
| overflowX | documentElement 横向溢出像素（布局爆行探针） | 28/28 为 0 |
| htmlDark | html.dark 与目标主题一致性（主题跟随） | 28/28 一致 |
| rawKeys | 界面残留 i18n key 文案（语言完整性探针） | 28/28 为空 |
| appErr | 应用内错误缓冲 recentErrors() 条数 | 28/28 为 0 |

逐格明细（7 路由 × 4 组合全部 PASS，证据截图见 .gui-agent/matrix/，文件名
`<n>-<路由>-<主题>-<语言>.png`，与 matrix-report.json 逐格对应）：

| 页面 | light zh-CN | dark zh-CN | light en-US | dark en-US |
| --- | --- | --- | --- | --- |
| 仪表盘 dashboard | PASS | PASS | PASS | PASS |
| 节点 peers | PASS | PASS | PASS | PASS |
| 事件 events | PASS | PASS | PASS | PASS |
| 发现 discovery | PASS | PASS | PASS | PASS |
| 中继 relay | PASS | PASS | PASS | PASS |
| 诊断 diagnostics | PASS | PASS | PASS | PASS |
| 设置 settings | PASS | PASS | PASS | PASS |

深浅主题下文本/边框/图表配色读自同一套 oklch token（见 §二），无固定色值导致
的暗色可读性问题；28 格无未交代项。

## 二、静态走查结论

1. **token 层完整**：index.css 定义 light/dark 全量变量对（background/card/
   popover/muted/accent/border/input/ring + chart-1..5 + sidebar 全套 +
   success/warning/info），`@theme inline` 全部映射进 Tailwind 色板。
2. **portal 组件跟随主题**：dialog content 用 bg-background，dropdown-menu
   content 用 bg-popover text-popover-foreground，alert-dialog/select 同模板同
   token；Radix portal 挂载于 document.body，`.dark` 类在 documentElement 上，
   级联天然覆盖 portal。
3. **sonner 跟随主题**：AppToaster 传 resolvedTheme 并把 --normal-bg/text/border
   映射到 --popover/--border 变量（components/ui/sonner.tsx）。
4. **图表配色**：sparkline 仅用语义 token（stroke-success/stroke-warning、
   border-success/40 等），无常量色。
5. **硬编码扫描**：components/views/routes 无 hex/rgb/rgba 字面量（grep 为零）；
   hardcoded-copy 门禁（vitest）保持绿。
6. **主题实现**：ThemeProvider 切 documentElement 的 .dark 类 + localStorage
   持久化，system 跟随 matchMedia；异常路径有 console.warn 可观测信号。

## 三、即时性探针（不刷新页面）

- **主题即时性**：light 下 body computed backgroundColor = oklch(1 0 0)，加
  .dark 后立即变 oklch(0.145 0 0)——token 级联即时翻转（含 portal）。
- **语言即时性**：设置页外观卡点击 English/中文（真实用户路径，
  i18n.changeLanguage），页面文案立即切换且来回可逆，无旧语言残留。

## 四、发现与处置

| # | 发现 | 根因 | 处置 |
| --- | --- | --- | --- |
| F1 | 打开 AlertDialog 时控制台警告 "Function components cannot be given refs"，栈指向 components/ui/alert-dialog.tsx:44（AlertDialogOverlay） | alert-dialog.tsx 全部包装组件为普通函数组件未 forwardRef；Radix Primitive 向 Overlay/Content/Action/Cancel 透传 ref 被丢弃，影响焦点归还等内部行为 | ⛔ 仅记录：components/ui 仅 input.tsx 在本轮边界内；建议独立小任务照 input.tsx 同法补 forwardRef 并加 ref 断言测试 |
| F2 | reset-identity-dialog 原裸按钮+手工 try/catch（S2 审计矩阵遗留） | 手工 resetting state、无 loading 图标/进行中文案 | ✅ 本轮改造：AsyncButton 三态 + 双语 resetting key + 四项测试 |
| F3 | Input 不转发 ref 致 react-hook-form register 回显失效（S1 审计记录） | React 18 函数组件 ref 被静默丢弃 | ✅ 本轮修复：forwardRef + 三项测试（修复前红/修复后绿留档） |
| F4 | factory-defaults.ts 与 config.rs 默认端点镜像无对表手段 | 双侧可各自静默漂移 | ✅ 本轮加固：factory-defaults.test.ts 直接解析 config.rs vec! 字面量对表，漂移即红（红绿证据见提交） |
| F5 | 走查探针首版把英文界面普通句子误报为 rawKeys（检测正则 . 退化为任意字符） | 驱动脚本转义层损耗，非应用问题 | ✅ 探针修正为 [.] 字面量后 28 格复跑全绿；应用侧无问题 |

## 五、未修项与待追认

1. ⛔ alert-dialog.tsx（含 dialog.tsx 同模板家族）补 forwardRef 未做——文件边界
   外（红线仅放行 components/ui/input.tsx），建议独立小任务收口。
2. 待追认：新增 key `settings.identity.resetting` 放在 settings.identity 既有
   命名空间（与 resetDone/resetFailed 同族），未按"新增 key 放 common"字面执行；
   理由：同族 key 就近维护优于跨命名空间引用。
3. 走查基于 mock IPC 数据形态；真实后端极端数据（超长 PeerId/地址串）下的溢出
   表现建议在联调环境补一轮抽查。

## 六、证据留档

- 双主题 28 格截图 + matrix-report.json：`.gui-agent/matrix/`（.gitignore 内，
  本地留档；报告 JSON 与本文档矩阵一一对应）。
- 即时性探针输出：themeFlips=true，langSwitchImmediate=true（见 §三）。
- 走查驱动脚本（零依赖 CDP，/tmp 运行不入库）：7 路由×2 主题×2 语言单
  Chrome 会话批测，逐格刷新+静置+探针+截图。
