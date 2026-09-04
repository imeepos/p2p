# ISSUE

## AGENTS.md 远端名与实际不符（2026-09-04 T36 检查轮发现）

- **信息不准**：AGENTS.md「收尾四步」一节写「远端名是 gitea 不是 origin」并示例 `git push gitea <分支>`；但本仓库实际只配置了 origin 远端（git@github.com:imeepos/p2p.git），不存在 gitea。账本 note（2026-09-04 12:30 CLI 对等波勘误）已明确「本仓库远端实为 origin（无 gitea），后续任务书统一用 origin」。
- **正确做法**：推送/删远端分支一律用 `git push origin ...`；AGENTS.md 待同步更正。

## 底座 facade 契约缺口：ProtocolHandler 拿不到入站流 PeerId（2026-09-05 ACP2 发现）

- **信息缺失**：crates/p2p-protocol 的 `ProtocolHandler::handle(&self, stream)` 只回调流，不传远端 PeerId；而 crates/p2p-swarm 的 serve.rs 在 dispatch 时明明持有 `peer`（喂给了 liveness/事件，唯独没进 handler）。设计（acp-over-p2p-design.md §4.1 ①）要求桥按「传输层互认 PeerId」查策略表，目前上层 app 无法直接拿到。
- **ACP2 的绕行**：apps/acp-agent/src/peers.rs 以 Node 事件（PeerConnected/PeerDisconnected）维护在线集，恰一 peer 在线才归属，空集短等、多 peer 歧义一律 fail-closed 拒绝并审计；单操作者场景正确，多操作者并发控制台会被误拒（有日志）。
- **期望修法**：底座把 peer 随流下传（trait 加参或 BoxedStream 携带元数据），acp-agent 删 peers.rs 直连真实身份；涉及 crates/**，非 ACP2 文件域，留待底座卡。

