# 远程桌面控制波 · 计划 v1（2026-09-15）

## 目标（用户直接指令）
在 p2p 底座上实现远程桌面控制，覆盖 rustdesk 核心功能（鼠标/键盘/剪贴板/文件传输等）
至商用程度。设计真值源：docs/design/remote-desktop-plan.md。

## 里程碑（每里程碑独立收官）
- M1（本轮）：设计定稿 + crates/rd-wire 线协议 crate（control/video/file 消息编解码 +
  帧 I/O，19 单测）+ 四协议注册（registry.toml/specs/wire-protocol.md §3.2）。
- M2：host 屏幕采集（ScreenCaptureKit）+ video 通道 + viewer 解码渲染 + 双节点 E2E。
- M3：输入注入（CGEvent 鼠标/键盘/修饰键）+ input 通道。
- M4：剪贴板双向同步。
- M5：文件传输（浏览/上下传/进度/取消/续传/路径卫生落地）。
- M6：商用收口：多显示器/质量自适应/重连/审批 GUI/审计/authz+服务开关接线/CLI 对等。
- M7（可选）：音频 + 硬件编码。

## 分支/scope
- 分支前缀 feat/rd-*；本轮 feat/rd-wire，worktree .worktrees/rd-wire。
- scope 登记见 .agents/collab/SESSIONS.md（2026-09-15 认领行）。
- 禁触：wsm 服务总控波（feat/wsm-b2 在飞）、tunnel 域（已收官，只读复用）。

## 依赖
- p2p-service 服务开关：wsm 波合入后接线 rd-host/rd-viewer。
- p2p-authz 能力 key（rd.control/rd.file/rd.clipboard）：M6 接线。