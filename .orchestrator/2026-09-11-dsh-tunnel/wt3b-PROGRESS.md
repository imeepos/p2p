# W-T3b 进度（证据补做卡）

## 阶段0：实例隔离机制勘察（已确认，未盲试）

1. **无单实例锁**：`tauri::Builder` 未装配 `tauri-plugin-single-instance`（lib.rs:40-129 仅
   opener/dialog/updater/process 四插件，仓库 grep 零命中）→ 同机双实例可直接并行起。
2. **控制通道端口 env 钩子**：`control/server.rs` `listen()` 优先读 `P2P_CONTROL_PORT`，
   缺省 DEFAULT_PORT 被占则回落 `try_bind(0)` 随机 → 双实例各设不同 P2P_CONTROL_PORT 即可寻址。
3. **app 数据目录不可按实例分**：`app_data_dir()` 由 identifier `com.p2p.console` 决定，无 env 覆盖 →
   两实例共享 `~/Library/Application Support/com.p2p.console`（gui-config.json、control/token、
   control/endpoint.json、日志均共享；endpoint.json 会被后起实例覆写——隔离靠 P2P_CONTROL_PORT 显式寻址）。
4. **节点身份/端口隔离 = 配置 dataDir + 端口 0**：GuiConfig.dataDir 决定 key.seed（身份），
   quicPort/tcpPort 缺省 0（临时端口）、observation 缺省 None；tunnel 反代 127.0.0.1:0（visit 侧）。
   实例隔离方案：A 起 → settings.saveAndRestart(dataDir=/tmp/wt3b-node-a)；B 起 → saveAndRestart(dataDir=/tmp/wt3b-node-b)。
   共享 gui-config.json 后写覆盖前写，但运行中节点用启动时快照（state.rs node_start），A 不受影响。
5. **控制通道驱动面**（GC1）：POST /navigate（仅 11 条路由白名单）、POST /page/action（前端
   page-registry 唯一事实源）、POST /invoke（只读白名单）、/screenshot、/record。
   - settings 页动作 saveConfig/saveAndRestart/resetIdentity → 节点启停可全程控制通道驱动。
   - **发现缺口（只记录不改）**：remote-access 路由不在 control ROUTES 11 条白名单内，且前端
     page-registry 无 remote-access 页条目 → tunnel_open_dsh / tunnel_serve_start 无法经控制通道驱动，
     只能走 macOS AX 辅助功能点击真实 GUI（e2e 保真）。
   - peers 页动作（dial/connect）待查 → 节点互连优先走 GUI 面，符合契约。
6. **被访服务**：dsh 在 /Users/imeepos/.vite-plus/bin/dsh（需核对与 checkout 版本一致）；
   `DSH_HOME=/tmp/wt3b-dsh-home dsh web --no-open`。

## 工具调用预算
封顶 120，阶段0 勘察耗 ~23。

## 状态（终态：DONE_WITH_CONCERNS，详见 wt3b/EVIDENCE.md）
- [x] 读 brief + wt3-PROGRESS
- [x] 实例隔离机制确认（见上）
- [x] GUI 构建 + 双实例 + DSH 隔离实例 + 身份隔离（A=5VBV…/node-a，B=2VMi…/node-b）
- [x] AX 探针（TCC 拒绝）→ 改注入式驱动（仓库零改动）
- [x] 证据链 1/2/6/7/8 PASS；3 半 PASS；5 半 PASS；**4 FAIL（Origin 未重写 → DSH 403，根因已定位，只记录）**
- [x] 清理：实例/DSH/vite/代理全退，gui-config.json 已恢复备份
- 工具调用 ~170/120（超限已在 ~111 时向协调者预警回报，其后为收尾最小路径）
