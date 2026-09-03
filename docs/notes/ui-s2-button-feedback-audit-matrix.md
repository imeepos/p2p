# W6-S2 按钮微交互与失败提示审计矩阵

> 范围：全 GUI 触发异步动作的按钮 × loading/success/fail/进度 × toast 覆盖。
> 结论标记：✅ 已具备 / 🔧 本轮改造 / ⛔ 边界外仅记录（views/settings/** 归 W6-S1）/ ➖ 纯同步（理由随行）。

## 仪表盘（dashboard + topbar）

| 按钮 | 异步 | loading | success | fail | 进行中文案 | toast（含原因/复制详情） |
| --- | --- | --- | --- | --- | --- | --- |
| 顶栏 启动/停止节点 | 是 | ✅ AsyncButton | ✅ 图标+toast | ✅ 图标+toast | 🔧 starting/stopping | 🔧 startFailed/stopFailed + errorText 原因 + 复制详情 + console.error 补齐 |
| 快速操作 启动节点 | 是 | ✅ AsyncButton | ✅ toast | ✅ toast | 🔧 starting | 🔧 startFailed + errorText + 复制详情（console.error 原有） |
| 快速操作 停止节点 | ➖ | - | - | - | - | 纯同步：仅打开二次确认弹框，IPC 在弹框确认按钮内 |
| 快速操作 拨号入口 | ➖ | - | - | - | - | 纯导航 Link |
| 停止弹框 确认停止 | 是 | ✅ AsyncButton | ✅ 关弹框+toast | ✅ toast（弹框留驻可重试） | 🔧 stopping | 🔧 stopFailed + errorText + 复制详情 |
| 演示卡 模拟异步 | 是 | ✅ AsyncButton | ✅ toast | ✅ toast | ➖ 演示 900ms 超短 | 🔧 失败演示带 description+context，展示复制详情 |
| 演示卡 toast/确认按钮 | ➖ | - | - | - | - | 纯同步触发（toast/confirm 演示本体） |

## 节点（peers）

| 按钮 | 异步 | loading | success | fail | 进行中文案 | toast |
| --- | --- | --- | --- | --- | --- | --- |
| 行内 Ping | 是 | ✅ AsyncButton | ✅ toast+rtt | ✅ fail 态+toast | ➖ 短动作（8s 超时上限内通常秒回，loading 图标即反馈） | 🔧 pingFail 原因 + detail（peer.ping 上下文）可复制 |
| 行内 详情 | ➖ | - | - | - | - | 纯同步：打开抽屉 |
| 工具栏/表尾 拨号入口 | ➖ | - | - | - | - | 纯同步：打开弹框/URL 参数 |
| 拨号弹框 提交 | 是 | ✅ AsyncButton | ✅ toast+逐跳面板 | ✅ 内联 commandError+toast | 🔧 dialing（新增） | 🔧 failed + 失败跳 detail/命令 errorText + 复制详情；console.warn 升级 console.error |
| 拨号弹框 关闭 | ➖ | - | - | - | - | 纯同步 |

## 事件（events）

| 按钮 | 异步 | loading | success | fail | 进度 | toast |
| --- | --- | --- | --- | --- | --- | --- |
| 暂停/恢复滚动 | ➖ | - | - | - | - | 纯同步：本地 snapshot 状态切换 |
| 导出 JSON | ➖ | - | - | - | - | 纯同步：Blob 下载无 IPC、无失败路径；成功有 exported toast |
| 清空 | ➖ | - | - | - | - | 同步链：confirm（弹框自带确认反馈）→ store setState，无 IPC；成功有 cleared toast |

## 发现（discovery）

| 按钮 | 异步 | loading | success | fail | 进度 | toast |
| --- | --- | --- | --- | --- | --- | --- |
| mDNS 开关 | 是（IPC） | ➖ | ✅ toast | ✅ toast | - | 🔧 saveFailed + errorText + 复制详情；Switch 非按钮组件，切换由配置回读驱动（按约定标注不套 AsyncButton） |
| 添加地址（弹框确认） | 是 | 🔧 AsyncButton（原手工 adding state） | ✅ 关弹框清空 | 🔧 fail 态中断（原因已由上游 toast/内联红字） | ➖ | 上游 persistBootstrap toast |
| 添加地址（入口） | ➖ | - | - | - | - | 纯同步：打开弹框 |
| 地址簿行内 删除 | 是（confirm+IPC） | 🔧 AsyncButton iconOnly（原手工 saving disabled，无 loading 图标） | ✅ 图标 | 🔧 fail 态（取消走 ACTION_CANCELLED_MARK 与 saveAndRestart 同约定；保存失败上游已 toast） | ➖ | 上游 persistBootstrap toast |
| 发现表 复制 PeerId | 是（clipboard） | ➖ | ✅ toast | ✅ toast | - | 复制为同步快操作，走共享 copyText；🔧 失败 toast 带原因 |

## 中继（relay）

| 按钮 | 异步 | loading | success | fail | toast |
| --- | --- | --- | --- | --- | --- |
| 中继地址 保存 | 是 | ✅ AsyncButton | ✅ toast+脏状态归零 | ✅ toast（校验失败走内联红字，流标记跳过 toast） | 🔧 saveFailed + errorText + 复制详情（context: relay.relayAddrs_save） |
| 加载失败 重试 | 是 | ✅ AsyncButton | ✅ 重新拉取 | ✅ toast | 🔧 补 errorText 原因 + context: config.get_retry（原 toast 无原因） |
| 水位/逐跳/降级链卡 | ➖ | - | - | - | 纯展示，无按钮 |

## 设置（settings）

| 按钮 | 异步 | loading | success | fail | 进行中文案 | toast |
| --- | --- | --- | --- | --- | --- | --- |
| 保存条 保存 | 是 | ✅ AsyncButton | ✅ 图标+toast | ✅ toast（校验失败内联红字不弹 toast） | 🔧 saving（新增） | 🔧 saveFailed + errorText + 复制详情（context: settings.config_save） |
| 保存条 保存并重启 | 是（confirm→save→stop→start） | ✅ AsyncButton | ✅ 图标+toast | ✅ toast（弹框留驻） | 🔧 saveAndRestarting（新增） | 🔧 restartFailed + errorText + 复制详情（context: settings.save_restart） |
| 加载失败 重试 | 是 | ✅ AsyncButton | ✅ | ✅ toast | ➖ | 🔧 同 load-state（共享组件） |
| 主题/语言选项 | ➖ | - | - | - | - | 纯同步：theme/locale 即时生效无 IPC |
| 网络卡 mDNS Switch | 是（IPC 随保存条提交） | ➖ | - | - | - | 表单字段，提交走保存条；不单独保存 |
| 身份卡 复制 PeerId | 是（clipboard） | ➖ | ✅ toast | ✅ toast | - | 复制同步快操作，走共享 copyText（⛔ W6-S1 文件，仅记录） |
| 身份卡 确认重置 | 是（IPC identity_reset） | ⛔ 手工 resetting state（有 disabled 防重入，无 loading 图标） | ⛔ toast | ⛔ toast | ⛔ 无 | ⛔ resetFailed + errorText；toastError 二参 string 由兼容层承接。**裸按钮+手工 try/catch 典型项，归 W6-S1 未改造，仅记录** |

## 诊断（diagnostics）

| 按钮 | 异步 | loading | success | fail | toast |
| --- | --- | --- | --- | --- | --- |
| 复制日志路径 | 是（clipboard） | ➖ | ✅ toast | ✅ toast | 🔧 统一走共享 copyText（原手写 promise 链无失败原因），失败带原因 |
| 刷新（错误缓冲/日志尾部） | 是（fire-and-forget IPC） | ➖ | ➖ | ✅ toast | 轻量读取 + 5s 自动轮询共用 load()，失败已有 toastError；刷新语义为触发后由数据到达体现，不加阻塞 loading |
| 清空错误缓冲 | ➖ | - | ✅ toast | ➖ | 纯同步：内存数组清空 |

## 全局组件

| 入口 | 异步 | 处理 | 说明 |
| --- | --- | --- | --- |
| 命令面板 复制 PeerId/地址 | 是（clipboard） | 🔧 失败 toastError 带原因 + console.error | CommandItem 非按钮组件，复制为同步快操作 |
| 监视 CopyButton（icon） | 是（clipboard） | 🔧 失败 toast 带原因；console.warn 升级 console.error | 复制同步快操作 |
| ErrorBoundary 重置 | ➖ | - | 纯同步 state 重置 |
| 侧栏折叠 | ➖ | - | 纯同步 |

## 失败 toast 升级（本轮核心）

- toastError(message, options)：`{ description?, detail?, context? }`，二参 string 兼容形态保留（W6-S1 文件零改动）。
- toast 内置「复制详情」action：剪贴板写入 `context: …` + `error: …` + `detail: …` 三行（操作上下文 + 完整错误），成功轻反馈「已复制到剪贴板」，失败 console.error +「复制失败」提示。
- 所有 S2 边界内失败路径统一 errorText(error) 作 description，杜绝 String(error) 的 "Error: " 前缀泄漏；console.error 全量补齐（G-H 错误管线采集）。

## 遗留与待追认

1. 保存并重启的阶段文案（写入→停止→启动）未逐阶段展示，统一为「保存并重启中…」；如需真阶段需给 save-bar 增加 props（本轮遵守 props 形状稳定约束），待追认。
2. ⛔ reset-identity-dialog 为裸按钮+手工 try/catch 典型项，归 W6-S1，未改造（toastError 兼容层保证其编译与行为不变）。
3. ping 的 loading 未加文字（短动作，图标足够）；dial 失败展示以失败跳 detail 为准，无失败跳时 fallback 文案 failedReasonUnknown。
