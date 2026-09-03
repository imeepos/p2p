# 只读走查结论存档（peer id / 监听地址 / GUI mock）

- 基线：main @ 1ebf980（2026-09-03 只读走查，未修改任何代码）。
- 缘起：GUI 显示监听地址 127.0.0.1 引发"是否应为 0.0.0.0"的疑问，顺带核查 peer id 持久化与开发态 mock 开关。三条结论均经源码逐一核实，供后续运维与 E9 参考。

## 1. PeerId 多次启动不变（前提：data 目录不变）

- 机制：PeerId = base58(sha256(ed25519 公钥))，身份与密钥绑定（crates/p2p-identity/src/lib.rs:3）；种子落盘 key.seed（32 字节，文件 0600、目录 0700），存在即加载、不存在才生成（crates/p2p-identity/src/seed.rs:14 load_or_generate）。
- 加载失败（损坏/长度不对）原样上抛、不静默重建新身份（crates/p2p-identity/src/seed.rs:13 注释），防止"重启换身份"的静默事故。
- 各入口：
  - facade 与 node 子命令：data_dir/key.seed，默认 ./p2p-data（crates/p2p/src/assembly.rs:91、crates/p2p/src/lib.rs:53、crates/p2p-cli/src/node.rs:61）；
  - bootstrap 子命令：与 facade 同一 key.seed，确保 PeerId 一致（crates/p2p-cli/src/bootstrap.rs:133）；
  - GUI：app 数据目录下 p2p-data/key.seed（apps/gui/src-tauri/src/config.rs:74）；
  - 例外：ping/discover 用临时目录，每次新身份，设计如此（crates/p2p-cli/src/ping.rs:79）。
- 会变的场景：换 --data 目录、删 key.seed/data 目录、默认相对路径下换启动 cwd。

## 2. 监听地址显示 127.0.0.1 是可拨号规范化，实际绑定 0.0.0.0

- 实际绑定恒为 0.0.0.0（QUIC+TCP 双栈，crates/p2p-swarm/src/swarm/mod.rs:88），跨机可达性不依赖显示字段。
- listen_addrs 由 local_addr() 归一化：未指定 IP（0.0.0.0/::）替换为 127.0.0.1，保证地址簿里的监听地址可直连（crates/p2p-swarm/src/swarm/config.rs:26 to_transport）。原因：0.0.0.0 不是可拨号目的地址，进入地址簿/打洞信令（crates/p2p-swarm/src/swarm/punch.rs:21）会拨号失败。
- GUI 只是如实展示后端返回值（apps/gui/src-tauri/src/state.rs:65 → node.listen_addrs()）；端口随机因 GUI 默认 quicPort/tcpPort 为 0。
- 展示语义提示：GUI 的"监听地址"实为"可拨号地址"，易被误读为仅监听 loopback；如需区分可在展示层标"bind 0.0.0.0:port + 本机可拨 127.0.0.1:port"（未实施，仅备忘）。跨机可达由地址观测 + rendezvous 注册可路由地址守卫负责（crates/p2p/src/assembly.rs:73 无可路由地址即告警，E5 复盘产物）。

## 3. pnpm run tauri dev 默认走 mock 数据

- 链路：tauri dev → beforeDevCommand pnpm dev → vite 开发模式加载 apps/gui/.env.development，其中 VITE_MOCK_IPC=1（文件内注释：开发态默认 mock，=0 强制真实 invoke）；apps/gui/src/lib/ipc.ts:22 据此切到 mockBackend，全部 invoke 与事件流均为模拟。
- mock 监听地址格式固定：0.0.0.0/34000~34500 随机 QUIC 端口、TCP = QUIC+1（apps/gui/src/lib/mock-ipc.ts:195）。
- 反证特征：若 GUI 显示 127.0.0.1 + 不连续大随机端口（OS 临时端口），说明看到的是真实后端（build 产物或 VITE_MOCK_IPC=0），不是 mock。
- 开发态接真实节点：VITE_MOCK_IPC=0 pnpm tauri dev，或写 apps/gui/.env.development.local；生产构建不受影响，恒走真实 invoke。

## 备注

- 本仓库唯一远端为 origin（github），无 gitea 远端；AGENTS.md 中"远端名是 gitea"规则不适用于本树，推送一律走 origin。
