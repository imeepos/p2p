# W-T5 EVIDENCE：headless serve 面 + busy 栅栏对账 + 102 跨机实证

日期：2026-09-11（本地时区 America/Los_Angeles）；分支 feat/wt5-headless-serve。

## 红绿纪律（段②）

- **RED（裁决前）**：`RED-busy-default.txt`
  - `cargo test -p p2p-tunnel --lib config` → `default_max_concurrent_covers_wt3c_burst_peak` FAILED（默认 4 ≠ 裁决 16）
  - `cargo test -p p2p-itest --test tunnel_busy_burst` → `default_gate_absorbs_wt3c_burst_peak_without_busy`
    FAILED：**busy=10 ok=4**（默认许可 4 下 wt3c 峰值 14 并发，超 4 部分必 busy，与 EVIDENCE-wt3c.md 实证一致）
- **GREEN（裁决后）**：默认 4→16 后两探针全绿；语义探针（许可 2 + burst 4 → 恰 2 served +
  2 busy，每条 busy 落 `Rejected(Busy)` 终态审计）绿；回归 tunnel_wave / tunnel_assembly_wire /
  llm-share 三 itest 套件零回归。

## 102 跨机全链（段④）

环境：102 = 无头 Debian 13（192.168.0.102，enp7s0f1=192.168.0.102/23）；仓 /tmp/p2p @
6204c4c6（wt5 分支，git bundle 经 scp 传输后 fetch，origin 仍指 GitHub）；cargo build
`--manifest-path apps/cli/Cargo.toml`（apps/cli 独立 workspace）；DSH 0.1.0-rc.6（npm 全局），
DSH_HOME=/tmp/wt5-dsh-home，`dsh web --port 13080`（systemd-run --user 单元 wt5-dsh，仅
loopback）；被访侧 `p2pctl tunnel serve --target 127.0.0.1:13080 --data-dir /tmp/wt5-serve-node`。

| 验收项 | 证据 |
|---|---|
| 两节点跨机互连，peerId 各异 | 访侧(mac)=`7DhU9rUD46bELUcF82rpWiTCWwfns1jDxWEtCYBEBGAb`；被访(102)=`67R7k4Q6DUfA6q65b18iWL5oFHpUqsz2z3xDiVwdQFL5`（ready 行）；GUI peers 页：67R7k4…QFL5 来源「局域网」状态「已连接」，传输地址 `240e:391:ecf:6330:130:d9f1:458f:99ad/u49759 +3`（非回环，mDNS 跨机发现） |
| 数据面全通（rc.6 无 WS 101，见偏差说明） | 浏览器经本地反代 `127.0.0.1:62377` 全量加载 DSH Web：46 静态资源 + 全部 `/api/*` POST 200（host.describe/settings.describe/session.create/session.prompt 等，playwright network log）；`/plugins/events` 长连流经隧道；mac 侧 tunnel_status：**178 会话（ok 164 / open 6 / io 8），bytesIn 合计 8,900,347** |
| 真实流式对话 | `wt5-cross-machine-chat.png`：会话「一句话回答1+1等于几」，模型 GLM-5.2，回答「1+1等于2。」，用时 4 秒 · 首 token 4.9s · 100 tok/s · 输入 7.3K tok（对话全程经隧道到 102 的 DSH→bigmodel 上游） |
| 双面审计留痕 | 102 侧 `evidence-102-serve.txt`：391 条 `tunnel session audit`（八字段全：session_id/peer_id/target/started_at/ended_at/bytes_in/bytes_out/outcome），peer_id 均为 mac 访侧 7DhU…；mac 侧 tunnel_status sessions 178 条同构八字段（camelCase） |
| loopback 监听证明 | `ss -tlnp`：DSH 仅 `127.0.0.1:13080` LISTEN（evidence-102-serve.txt） |
| 负例 | `evidence-negative-403.txt`：穿隧道 POST /api/host.describe，`Origin: http://evil.example` → **403 forbidden**；`Origin: http://127.0.0.1:60491`（反代口，非受信 authority）→ **403** |
| 优雅收口 | `evidence-102-graceful-shutdown.txt`：SIGTERM → gate 关（新流 `outcome=rejected:shutdown` 逐条落终态审计）→ 浏览器长连占住 5 条在途会话 → 10s 收口超时 → 显式报错「收口超时（10s）：仍有 5 条在途会话未落终态」非零退出（禁静默）；无在途会话的干净收口（ready→stopped 行、exit=0）见段① live 冒烟 |

## 偏差登记（DSH 版本事实，非隧道缺陷）

1. **WS 101 → 不适用**：102 上 DSH 为 0.1.0-rc.6，数据面为 `/api/*` POST RPC（session.prompt
   200 流式响应）+ `/plugins/events` 长连 GET，全站无 websocket 端点（network log 证）。
   wt3c 时代的 `/api/remote.mux` WS 101 在该版本不存在；实时双向数据面等价物=长连 events 流
   + 流式 prompt 均经隧道贯通。
2. **坏 token 401 → 403**：rc.6 鉴权为浏览器信任栅栏（authority/Origin 校验），无 boot token
   机制（启动日志无 token、CLI 无 token 参数）；负例按同语义取「非受信 Origin → 403」。
3. GUI 要求启动 URL 带 token 参数（契约 §19.3 缺 token → Err），rc.6 不校验该参数——按透传
   语义用占位 token `?token=wt5`，未参与任何鉴权。

## 过程资产（复跑用）

- `driver.js` / `driver-proxy.mjs` / `static-injector.mjs`：webview 注入驱动链（wt3b 协议
  兼容；vite 在本环境长跑不稳，生产 dist + 静态注入服务器替代；driver 补丁：identify 3s 竞速
  + boot 必达）。GUI 二进制须 ≥ wt3c Origin 重写修复（3635fb47）。
- 102 侧残留清理：`systemctl --user stop wt5-dsh`；/tmp/wt5-dsh-home、/tmp/p2p、/tmp/wt5-serve.*
  可删。
