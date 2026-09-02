# p2p GUI 总体规划（Tauri + React）

状态：v1（2026-09-02，协调会话制定）；契约细节见 [gui-contract.md](gui-contract.md)；派单与进度见 [../gui-coordination.md](../gui-coordination.md)。

## 1. 定位与范围

p2p-base 已有完整通信内核（身份/传输加密/发现/穿透中继/降级链/metrics）与 CLI。本应用只做一件事：
把 Node 的能力可视化——**本机节点的启停与配置、邻居节点的发现与连接、降级链路径可视化、事件流观测**。
纯可视化 GUI，不新增协议语义、不动 crates/ 既有代码（只读依赖）。

非目标：聊天/文件传输等业务应用、多节点编排、远程管理其他机器的节点。

## 2. 同类产品调研结论

| 参照 | 采纳 | 摒弃 |
|---|---|---|
| qBittorrent | 左侧导航 + 顶栏 + 底部状态栏三段式；表格主视图 | 拥挤的工具栏图标堆砌 |
| Transmission | 简洁列表 + 检查器（inspector 抽屉看单节点详情） | 弱搜索 |
| Syncthing Web | 卡片式仪表盘、设备 ID 展示/复制、事件面板、设置分组表单 | 层级过深的侧栏 |
| WireGuard/Tailscale 客户端 | 状态灯语义（绿=已连接）、一键开关节点 | —— |

共识：P2P 节点管理 = **状态总览 + 节点表 + 事件流 + 设置表单** 四件套；路径降级链可视化（直连→打洞→中继）
是本项目独有的差异点，必须做成显式 UI（节点详情/ ping 结果里逐跳展示）。

## 3. 信息架构（菜单注册制）

菜单走登记文件 `apps/gui/src/config/menu.def.ts`（append-only，注册项变更独立小提交）：

| 路由 | 菜单 | 内容 | 数据来源（命令/事件） |
|---|---|---|---|
| `/` | 仪表盘 | 状态卡（运行/PeerId/监听地址/运行时长）、指标卡（发现/连接/中继会话/门禁拒绝）、拨号跳成功率、最近事件 | node_status + metrics_get + node-event |
| `/peers` | 节点 | 已知节点表（PeerId/地址/来源/状态/最后活跃）；操作：拨号、ping、复制 PeerId；检查器抽屉展示逐跳历史 | node_status、peer_dial、peer_ping、node-event |
| `/discovery` | 发现 | mDNS 开关与结果、rendezvous bootstrap 地址簿（增删、注册/查询手动触发） | GuiConfig、node-event(peer_discovered) |
| `/relay` | 中继 | relay 地址配置、会话水位、逐跳成败统计（DialHop 聚合） | GuiConfig、metrics_get |
| `/events` | 事件 | 实时事件流：类型过滤、文本搜索、暂停/滚动、清空、导出 JSON | node-event |
| `/settings` | 设置 | 节点配置表单（端口/mdns/数据目录/宣告地址/观测）、外观（主题/语言）、身份（PeerId/数据目录/重置身份——危险操作） | config_get/save、node_stop |

全局壳：侧栏（图标+文字，可折叠）+ 顶栏（节点状态 pill、启停按钮、主题切换、语言切换）+ 底部状态栏
（运行状态点、监听端口、活跃连接数、版本）。

## 4. 布局系统

- **12 等分 CSS 网格**（业界通用 12 列）：内容区 `grid grid-cols-12`，卡片跨度 3/4/6/12；小屏自动降级 6/12。
- 间距 8px 基准（Tailwind 默认 spacing，常用 4/6/8 单位 = 16/24/32px）。
- 三行主框架：顶栏 h-14、内容 flex-1（内滚动）、状态栏 h-8；侧栏 w-60（折叠 w-14）。
- 断点：sm 640 / md 768 / lg 1024 / xl 1280；窗口最小 960x600。

## 5. 反馈系统三级规范（硬性）

| 级别 | 组件 | 使用场景 | 规范 |
|---|---|---|---|
| 微反馈 | AsyncButton（内置 idle/loading/success/fail 四态图标动画） | 一切异步按钮（启停节点、拨号、ping、保存配置、增删地址） | loading 旋转 ≥300ms 防闪烁；success/fail 图标驻留 1.2s 后回 idle |
| 轻反馈 | toast（sonner 风格） | 异步命令结果、事件通知（peer_connected/listen_failed 等） | 成功右上 3s；失败 6s 可重试；同类 3s 内去重合并；事件 toast 可在设置关掉 |
| 弹框 | Dialog / AlertDialog | 危险操作确认（停止节点、重置身份、删除 bootstrap 地址）、表单（拨号、添加地址） | 危险操作必须 AlertDialog 二次确认并显示影响说明；表单校验失败内联红字，不弹框 |

失败路径禁止静默：命令 Err 必须落到 toast + 事件面板双通道。

## 6. 主题与国际化

- 主题：light / dark / system 三选一，CSS 变量 token（shadcn 语义色），class 策略 + system 监听；
  切换零闪烁（启动内联脚本预读 localStorage）。
- 语言：zh-CN（默认）、en-US；i18next + 类型安全 key（`src/i18n/locales/{zh-CN,en-US}.ts`，注册变更独立小提交）；
  key 命名空间按视图划分（dashboard.* / peers.* / settings.* / common.*）。
- 所有时间/字节/速率格式化走统一 util（Intl），随语言切换。

## 7. 技术选型

- 壳：Tauri 2（Rust 后端，`apps/gui/src-tauri`，独立 cargo 项目，根 workspace exclude）。
- 前端：React 18 + TypeScript + Vite + Tailwind CSS 4 + shadcn/ui（Radix）+ react-router + i18next + lucide-react。
- 状态：视图本地 useState/useReducer + 一个 NodeStore（zustand，订阅 tauri event 单例）。
- 目录：`apps/gui/`（前端根）+ `apps/gui/src-tauri/`（Rust）。前端不 import 任何 rust 代码；rust 不写 HTML。

## 8. 波次计划与依赖关系

```
W1 骨架（并行）
  A tauri 桥接骨架 ──┐
  B 前端骨架(含mock) ─┴─→ W2 视图（并行）
                          C 监控视图(仪表盘/节点/事件) ──┐
                          D 配置视图(设置/发现/中继)   ──┴─→ W3 集成+打磨
                                                            E 集成联调(双节点真实冒烟) ─┐
                                                            F 打磨(a11y/i18n/主题/空态) ─┴→ W4 打包验收
                                                                                              G tauri build + README + 回归
```

依赖逻辑：契约（gui-contract.md）先行冻结 → A/B 对同一契约编程互不等待；视图依赖骨架的路由/主题/i18n/反馈组件
与真实 IPC；集成依赖全部视图；打包依赖集成。每波机械验收命令见 gui-coordination.md。

## 9. 验收门禁（每波硬性）

1. Rust：`cargo clippy -- -D warnings` 零告警 + `cargo test` 全绿（src-tauri 内）。
2. 前端：`pnpm -C apps/gui build`（含 tsc 类型检查）零错误。
3. 文件红线：≤300 行/文件、≤60 行/函数；无 emoji；失败路径有日志/错误信号。
4. 行为红线：不改 crates/**；不改 docs/coordination.md；注册类文件（menu.def.ts/i18n/locale）变更独立小提交。
5. 最终验收：双节点真实互通演示（本机两实例 mDNS 发现 + echo ping 逐跳展示）+ `pnpm tauri build` 产物可启动。
