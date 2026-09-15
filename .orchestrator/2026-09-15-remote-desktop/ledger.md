# 2026-09-15-remote-desktop 波次账本

## 认领
- 2026-09-15 session-rd-goal（goal round 主会话）认领；SESSIONS.md 留痕。

## M1（本轮）执行记录
- 2026-09-15 12:13 设计定稿 docs/design/remote-desktop-plan.md v1（commit 39a6f44a）。
- 2026-09-15 12:13 crates/rd-wire 落地（commit b2a96281）：control/video/file 消息编解码
  + 帧 I/O；19 单测绿；clippy -D warnings 零告警。
- 2026-09-15 12:13 四协议注册（commit d4be90d3）：/rd/control/1、/rd/video/1、/rd/file/1
  implemented + /rd/audio/1 planned；protocol-registry 四向 PASS（21 id）。
- 2026-09-15 12:30 README/docs 索引条目（docs 装饰小提交）。
- 门禁：line-limit/fmt/panic-hygiene/protocol-registry 单跑 PASS；全量 cargo test
  --workspace 后台执行中（共享主树 target 缓存）。

## M2（本轮）执行记录
- 2026-09-15 crates/rd-capture：CaptureSource trait + SyntheticSource + SCK feature 门控
  （sck 默认关，本机无 Swift 运行时；sck 编译通过）。
- 2026-09-15 crates/rd-host：control 握手+控制循环 / video 帧泵（raw+zlib 编码、keyframe
  每 30 帧、fps 节流、空闲超时护栏、同 Peer 单活跃会话）。
- 2026-09-15 crates/rd-viewer：拨号握手 / 视频泵解码（raw+zlib）/ keyframe 同步 /
  RenderSink 接缝 / 显式 close。
- 2026-09-15 crates/p2p-itest/tests/rd_video_wave.rs：3 项 E2E 绿（raw 全链、重复拨号拒绝、
  zlib 往返）。门禁：fmt/clippy -D warnings 零告警（rd-wire/rd-capture/rd-host/rd-viewer/
  p2p-itest 五 crate 全量）。

## 里程碑状态
- M1：已收官合并（main @ dc870038）。
- M2：实现完成，待全量门禁绿后合并。
- M3：输入注入（cgevents/CGEvent）沿控制通道接入；见 plan.md。

## 依赖挂账
- p2p-service 服务开关（rd-host/rd-viewer）：wsm 波（feat/wsm-b2）合入后接线。
- p2p-authz 能力 key：M6 接线。
