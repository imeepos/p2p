# TD 轮进度：GUI 通用入口 tunnel_open + remote-access 页泛化

状态：DONE_WITH_CONCERNS（全门禁绿；live 桌面双节点截图链未做，证据以单测+组件测试落档）
分支：feat/wtd-gui-open（worktree .worktrees/wtd-gui，基于 origin/main 96514234）
预算：120 次调用（分段 40/80 回报已发；终报本文件落档后 send_parent）

## 产出物

1. src-tauri：
   - `tunnel/url.rs`：新增 `parse_generic_target`（`127.0.0.1:<port>` 字面量校验，
     先校验后动作；token 恒空串）+ 2 条单测（合法/非字面量/坏端口闭集）。
   - `tunnel/audit.rs`：`open_url` 空 token 分支 = 不拼 `?token=`，open_url 即
     local_addr 本身（§19.3-9；DSH 形态 token 恒非空，行为零变化）。
   - `tunnel/visit.rs`：`tunnel_open(app, state, target, peer)`：parse_generic_target
     → parse_peer（复用）→ NodeTunnelOpener 装配（复用）→ TunnelState::start（复用，
     stop-旧-start-新 单会话不变）；不自动开浏览器。
   - `tunnel.rs`：`#[tauri::command] tunnel_open` 薄包装（两段路径，禁三段 KI:601）。
   - `lib.rs`：命令表 `tunnel::tunnel_open,` 插在 tunnel_open_dsh 行后（既有四行序不变）。
2. cli-parity.tsv：+1 行 `tunnel_open	exempt`（引 §19.3-8 进程活语义：headless
   对等面 p2pctl tunnel connect 非同一对象；token="" 无浏览器开窗语义）。
3. 前端：
   - `lib/ipc-types.ts` + `lib/ipc.ts`：`tunnelOpen(target, peer)` IPC 面；
   - `lib/mock-ipc.ts`：mock 显式报错（不假装可用）。
   - `views/remote-access/generic-tunnel-card.tsx`（新）：通用服务卡片，端口输入
     （固定前缀 127.0.0.1:）+ peerId 输入 + 打开按钮；空态禁按钮/加载态/错误态
     role=alert（复用 remoteAccess.error.* 文案）三态齐全。
   - `views/remote-access/remote-access-view.tsx`：挂载卡片，成功/失败汇入共享
     状态卡（local_addr + 可复制链接复用既有 copyUrl）；DSH 入口交互与文案零改动。
   - `views/remote-access/generic-tunnel-card.test.tsx`（新）：3 用例覆盖
     空态禁用 / 成功（断言 `127.0.0.1:3000` 入参 + local_addr/openUrl 展示 + 复制按钮）
     / 错误（坏 target 服务端报错进 alert）。
4. i18n：`remoteAccess.generic.*` 九键 zh-CN/en-US 双登记（types.ts 自 zh-CN 派生，
   无需手改）；独立小提交。
5. 文档对账：docs/ops/p2pctl-ai-guide.md 补 `### p2pctl tunnel connect` 条目
   （TC 落地遗留缺口，非本轮命令面；ai-docs-sync 门禁要求，独立提交）。

## 门禁证据（命令 + 真实退出码）

| 门禁 | 结果 |
|---|---|
| cargo test -p p2p-console（worktree src-tauri 内） | EXIT=0（lib 141 过；tunnel 面 9 过 = 7 旧 + 2 新） |
| cargo clippy -p p2p-console --all-targets -- -D warnings | EXIT=0 |
| pnpm lint（apps/gui） | EXIT=0 |
| pnpm test（apps/gui） | EXIT=0（232 文件 1411 用例全过，含新 3 例） |
| pnpm build（apps/gui） | EXIT=0（chunk>500kB 警告为存量） |
| bash scripts/check/cli-parity.sh | EXIT=0（GUI 命令 83，映射 69，豁免 14，CLI-PARITY-OK） |
| bash scripts/check/ai-docs-sync.sh | EXIT=0（AI-DOCS-OK；补 connect 条目后） |
| bash scripts/check/line-limit.sh | EXIT=0 |
| apps/gui bash scripts/check/i18n-diff.sh | EXIT=0（zh=1618 en=1618） |

## 红线对账

- 界面调用点：`generic-tunnel-card.tsx` 表单提交 → `ipc.tunnelOpen(...)` →
  `invoke("tunnel_open")`，组件测试断言入参与三态渲染，非零调用点。
- git diff main --name-only 全部落在 apps/gui/（src-tauri+src）+ scripts/check/
  （cli-parity.tsv）+ docs/ops/p2pctl-ai-guide.md + .orchestrator/2026-09-12-tunnel-any/。

## Concerns（终报同步上报）

1. 截图证据链未产出：成功态需桌面 app + 可达双节点（Tauri invoke + 真实隧道），
   浏览器 dev 模式 mock 面显式报错不假装可用（mock-ipc 守卫），故以组件测试
   （jsdom 断言成功/错误两态 DOM）+ 单测替代；如需真机链由 TB/TC 环境补拍。
2. ai-docs-sync 初跑 FAIL 为 main 存量缺口（TC 的 tunnel connect 无文档条目），
   本轮补齐（独立 docs 提交），不属于契约文本改动。
3. vitest 在 harness shell 需以真实 node runtime 置顶 PATH（vite-plus shim 挂起坑，
   已录 known-issues #372 同款）；worktree 内 node_modules 为本树真实安装（16s）。

## 复盘

- 照抄装配路径策略有效：visit.rs 参数化面（TB 移交）使 tunnel_open 落地仅 ~35 行；
  audit.rs 空 token 分支是唯一语义缝，契约 §19.3-9「禁拼 ?token=」由该分支集中满足。
- 行数红线贴近：visit.rs 266→~296（<300），后续再加命令应考虑拆文件。
