# W-T3 tunnel 访侧进度

- worktree: .worktrees/wt3-client，分支 feat/wt3-tunnel-client（i18n/rust/视图/登记/2×fix 已提交；§19 重构待提交）
- 终态: DONE_WITH_CONCERNS，分支已 rebase 最新 origin/main 并 push
- 消费 W-T2 完成：删本地 client.rs/ticket.rs，改 p2p_tunnel::{TunnelClient,TunnelTicket,TunnelErrorCode,PROTOCOL_ID}；
  NodeStreamFactory(Node::open_raw_stream 裸流) + TunnelOpener trait 缝；模块重排 tunnel.rs(根)+tunnel/visit.rs(访侧)
- 门禁终值: 前端 lint/test(1408)/build/i18n 四绿；cargo test p2p-console 全过(含 tunnel 15 用例)；clippy 双 crate 0；
  panic-hygiene PASS(216 文件)；CLI-PARITY-OK(tunnel 四行 exempt)；行数红线全达标(≤275)
- CONCERNS: ①本机端到端实证待补做轮（双 GUI 实例集成，dsh web 隔离实例已验证可起+401 负面已实测）；
  ②pump 在 TcpStream wire 半关时序疑点转 W-T4；③peer 输入参数待 W-T1 对齐
- §19 重构纪要（协调者 2026-09-11 指令）:
  - types.rs 重写为 TunnelStatusReport { active, localAddr, target, sessions, serve }；
    serve 字段 emit 恒带（W-T2 接线前默认关闭态，set_serve 接线位已留）
  - audit.rs 新增：ConnAudit 八字段（sessionId=票据 uid 同源/startedAt/endedAt/
    bytesIn/bytesOut/outcome 首写终态），AuditLog 进 status().sessions
  - client.rs open_tunnel 改签名（audit 注入，uid 同源 + 应答帧计 bytesIn）；
    proxy.rs 拆 pump.rs（forward_exact/drain_reply 逐块计字节）
  - 前端 ipc-types/ipc/mock-ipc/视图/测试同步 §19 形状；openUrl 由命令返回值本地保留
- 分工确认: W-T2 出 crates/p2p-tunnel（TunnelClient/TunnelResponder/LocalProxy 接口）
  + GUI 进程内 responder 装配；W-T3 消费其导出做访侧反代与 GUI 入口
- 已完成:
  1. [x] worktree 创建 + PROGRESS.md
  2. [x] 勘察（acp-pump/control/console/lib.rs/views/i18n/menu.def/palette-nav/StreamFactory）
  3. [x] Rust 访侧反代 tunnel/{proxy,head,client,ticket,url,types,mod}.rs
     - 127.0.0.1 字面量绑定 + port 0；Host 重写；Connection: close hop-by-hop 取舍
     - 流式逐块转发（64KiB chunk）；WS 101 后 copy_bidirectional 裸泵
     - 客户端按冻结契约 §1-§4 帧序（票据帧→ack/error→字节面）；TunnelDialer 缝
     - 单测: 真实回环 TCP 对端验 Host 重写/流式间隔/升级裸泵/502 透因
  4. [x] Tauri 命令 tunnel_open_dsh/tunnel_status + 事件 tunnel_status（lib.rs 两行，
     精确插在 authz_default_role_save 之后）；peer 为必需可选参数（契约 §7 未含，
     待 W-T1 §19 对齐）；tunnel_open_dsh 成功即经 opener 开系统浏览器
  5. [x] GUI 远程访问视图（三态）+ ipc 出口 + mock 同签名 + 路由/palette 登记
  6. [x] i18n zh-CN/en-US remoteAccess 命名空间（独立小提交）
- 提交序列: i18n → rust 命令面 → 视图/IPC → 路由登记 → fix 视图错误态 → fix shutdown_sync
- 待办:
  7. [ ] 门禁: cargo test/clippy（后台跑）; pnpm lint/test/build; check:i18n 已 PASS
  8. [ ] panic-hygiene PASS（已跑，只扫 crates/**；apps 内新增文件自检无 unwrap/expect 于非测试路径，除 bind 一处 expect(bound listener)）
  9. [ ] rebase main + push origin feat/wt3-tunnel-client
- 阻塞/依赖（按契约并行）:
  - p2p-tunnel 未落地 → 隧道真连端到端（浏览器→反代→隧道→responder→dsh web）
    的完整证据链待 W-T2 合并后补跑；本轮先交：dsh web 启动证据（隔离 profile
    wt3e2e，规避本机 web profile 的 ui-quick-input 重复登记故障）+ GUI 三态截图
  - 关闭命令缺失: 契约 §7 只冻结两命令；关闭走 RunEvent::Exit 收尾，
    建议 W-T1 §19/后续波次补 tunnel_close
