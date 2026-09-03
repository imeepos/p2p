# p2p-console 前端（Tauri + React）

p2p-base 内核的可视化控制台：节点启停与配置、邻居发现与连接、降级链（直连/打洞/中继）路径可视化、事件流观测。

## 开发

```bash
pnpm install            # 仓库根执行一次即可
pnpm -C apps/gui dev    # vite 开发服务器，端口 5173
```

- 开发态默认走 mock IPC（`.env.development` 的 `VITE_MOCK_IPC=1`）：模拟启动延迟、周期
  peer_discovered/connected/dial_hop 事件流与随机 rtt，无需 Rust 后端即可联调全部视图。
- 要连真实 Rust 后端：`VITE_MOCK_IPC=0 pnpm -C apps/gui dev` 并另起 `pnpm -C apps/gui tauri dev`
  （tauri dev 会拉起 src-tauri 桥接；mock 关闭后 ipc.ts 直连 tauri invoke，视图层零感知）。
- IPC 契约见 `docs/design/gui-contract.md`（冻结）；`src/lib/ipc.ts` 是唯一 IPC 出口。

## 测试与门禁

```bash
pnpm -C apps/gui lint         # ESLint（typescript-eslint + react-hooks + react-refresh）
pnpm -C apps/gui test         # Vitest 单测（AsyncButton/useConfirm/mock-ipc/event-reducer）
pnpm -C apps/gui build        # tsc -b 严格类型检查 + vite build
pnpm -C apps/gui check:i18n   # zh-CN/en-US locale key 集合机械对比
bash scripts/check/gui-gate.sh   # 仓库根：GUI 合并门禁（rust clippy/test + 前端 build）
```

约定：单文件 ≤300 行；i18n locale 只允许文件末尾按命名空间追加（append-only，
注册变更独立小提交）；menu.def.ts 注册制，新增视图先登记再挂路由。
## Agent 观测与操作入口（G-H）

前端错误不再只进 console：`src/lib/error-report.ts` 采集 window error /
unhandledrejection / console.error（含 ErrorBoundary），JSONL 落盘

- Tauri 环境：`~/Library/Logs/com.p2p.console/frontend.log`（超 1MB 轮转 .1），
  外部直接 `tail -f` 即可感知前端报错，无需打开 DevTools；
- 浏览器 mock 模式：降级 localStorage 键 `p2p-console.frontend-log`。

应用内入口：侧栏"诊断"页（/diagnostics）——错误缓冲、日志路径、持久化尾部。

页面操作入口（dev，默认 mock IPC）：

```bash
pnpm -C apps/gui dev                      # 先起 vite（或复用已跑的 dev server）
node scripts/gui-agent.mjs snap out.png   # 全页截图
node scripts/gui-agent.mjs errors         # 控制台+未捕获异常+应用内错误缓冲（JSON）
node scripts/gui-agent.mjs eval "1+1"     # 页面内任意 JS
node scripts/gui-agent.mjs click "button" # querySelector 点击
```

零依赖 CDP 实现（node 原生 WebSocket + 本机 Chrome，`CHROME_BIN` 可覆盖）；
dev 之外（打包版）在 URL hash 加 `agent=1` 同样暴露 `window.__P2P_AGENT__`。

## 打包

前置：Rust 工具链（cargo 在 PATH）、本仓库根 `pnpm install` 已执行。

```bash
# 1. 修改图标后重新生成全套（源图 src-tauri/icons/icon-source.png）
pnpm -C apps/gui exec tauri icon src-tauri/icons/icon-source.png

# 2. 打包（先跑前端 build，再 cargo release 编译并出 bundle）
pnpm -C apps/gui tauri build
```

产物路径（macOS，app/dmg targets）：

- `apps/gui/src-tauri/target/release/bundle/macos/p2p-console.app`
- `apps/gui/src-tauri/target/release/bundle/dmg/p2p-console_0.1.0_aarch64.dmg`

说明：

- 本地无签名证书时产物为未签名包（tauri 不配置 signing 即跳过）；分发需自行接入签名与公证。
- bundle 配置（targets/category/描述/copyright/icon）在 `apps/gui/src-tauri/tauri.conf.json`。
- 首次 release 编译耗时较长（编译 p2p 内核 crate 链），后续增量构建快。
