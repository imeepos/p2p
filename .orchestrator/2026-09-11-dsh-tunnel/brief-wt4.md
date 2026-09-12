# W-T4：流装配 double-write 清扫 + pump 半关复核 + llm-share 0.1.7 互通

## 背景
协议 ID 双写缺陷：`Node::new_stream`（crates/p2p/src/node.rs，首帧协议 ID 已写）被再包进
StreamFactory 时流上出现两帧协议 ID，严格 responder 翻车（W-T2 曾误诊为玄学 bad_ticket）。
2026-09-11 已在 main 落地两个原语：`new_stream`（调用方拿即写帧）与 `open_raw_stream`
（裸流，调用方自握手，见 node.rs 注释与 2026-09-11 协调者裁决）。本卡把装配面清扫到
单一正确口径，并以红绿 itest 固化。

## 范围（文件域独占，与并行卡 W-T3c 互斥）
- 只许改：crates/**（重点 p2p、p2p-swarm、p2p-tunnel、llm-share-proxy、p2p-itest）、
  apps/cli/**、docs/protocol/specs/tunnel.md 的装配 MUST 条款、相关契约注释。
- 禁改：apps/gui/src-tauri/**（W-T3c 域）；不加 GUI 命令；不动 cli-parity.tsv。

## 任务清单（分段推进，段末回报）
1. **清扫**已知包装点（先全仓 grep `impl StreamFactory`/`new_stream` 兜底，别只改这两处）：
   - apps/cli/src/llm_share/borrow_dial.rs:134
   - crates/p2p-itest/tests/tunnel_common/mod.rs 的 NodeFactory
   目标口径：工厂一律包 bare SwarmFactory（crates/p2p-swarm/src/swarm/streams.rs:17 契约）；
   调用方自握手的场景用 `Node::open_raw_stream`。严禁工厂内再包 `new_stream`。
2. **严格 no-tolerance 红绿 itest**：新增（或强化）itest 断言「流上首帧=协议 ID 恰一帧，
   其后首业务帧=票据 JSON」，红=装配双写必红，绿=清扫后全绿。responder 现存容忍
   （crates/p2p-tunnel/src/responder.rs read_ticket skip）按其注释原意保留为防御层，
   但 itest 不许依赖容忍路径制造假绿。
3. **pump 半关真实 TCP 复核（W-T3 移交 MUST）**：W-T3 测试实证「FIN 后对端写帧，
   pump 读端 0 字节」（duplex wire 无此象）。用真实 TcpStream wire 红绿定案：若 pump
   确有半关读缺陷，修 crates/p2p-tunnel pump 并带回归；若为测试接线artifact，写
   结论与证据进回报。冻结契约半关闭语义为准。
4. **llm-share 0.1.7 互通**：先跑既有 llm-share itest 定红绿基线再动手；保证清扫不破坏
   与 0.1.7 对端互通（crates/llm-share-proxy/src/wire.rs:21-29 容忍先例语义不变）。
5. **契约条款**：tunnel.md 加装配 MUST 一条（工厂包 bare SwarmFactory；自握手走
   open_raw_stream；禁止包装 new_stream）；stream_factory.rs/streams.rs 契约注释同步。

## 验收
- 清扫点全部迁移 + 新 itest 红绿双向证据；llm-share itest 与 0.1.7 互通绿；
- 触及 crate `cargo clippy --all-targets -- -D warnings` 零警告；panic-hygiene 红线不破；
- 自建 worktree 内跑全量 `make check`（CI 等效 locale：`CI=true LANG=en_US.UTF-8
  LC_ALL=en_US.UTF-8`）全绿后 push；分支 feat/wt4-stream-assembly；禁自合并。
- 迁移编号规则：若涉及迁移先查两处占号（本卡预计无）。

## 纪律
- 预算检查点分段：上述 5 段各段末回报；总顶 240 次调用，段末超支预警
  （前车之鉴：W-T3b 单总顶 120 被迫超到 171，分段回报避免一次性爆顶）。
- W-T3c 并行在 apps/gui/src-tauri 域作业：你 rebase 时若冲突仅可能在 docs/protocol/
  specs/tunnel.md，冲突一律你侧消化，保持其语义。
- 完成写 DONE 报（含红绿证据指针）回报派发者。
