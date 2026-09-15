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

## M3（本轮）执行记录
- 2026-09-15 crates/rd-input：USB HID→macOS 键码映射表（字母/数字/F1-12/修饰/导航，
  8 单测）、修饰键状态机（host 权威按住集合 + FlagsChanged 全量 flags）、
  RecordingInjector + 工厂（E2E）、MacInjector（core-graphics 0.24，AXIsProcessTrusted
  授权探测，CGEventSource 非 Send 故逐事件新建）；#[ignore] 真实注入冒烟。
- 2026-09-15 rd-host：控制循环输入分发（move→buttons diff→wheel；未映射键丢弃告警；
  断线/关闭必 reset_keys）；with_config 注入 factory 参数。
- 2026-09-15 rd-viewer：ControlWrite 便捷面 mouse/key/key_reset。
- 2026-09-15 crates/p2p-itest/tests/rd_input_wave.rs：真实双 Node 输入全链 E2E 绿
  （鼠标按下/松开/移动/滚轮 + 修饰键组合 + 全键重置，13 事件顺序断言）。
- 门禁：fmt/clippy -D warnings 零告警（五 crate）。

## M4（本轮）执行记录
- 2026-09-15 crates/rd-clipboard：ClipboardBackend trait + SystemClipboard（arboard 3，
  跨平台无 Swift）+ MemoryClipboard/Factory（共享 Arc，set_external 模拟外部变更），
  3 单测。
- 2026-09-15 rd-host：控制循环重构——帧读独立 reader 任务 + mpsc 队列（修 select!
  与 read_frame 竞态截帧，read_frame 非 cancel-safe）；剪贴板双向（viewer 消息写入
  本机；500ms 轮询 diff 上行 + last_seen 回声抑制）；握手期剪贴板/输入工厂失败显式
  hello_ack 拒绝；session.rs 拆 control.rs/sessions.rs（行数红线）。
- 2026-09-15 rd-viewer：connect_full 增剪贴板后端参数；控制流 split 读写半，读任务
  处理 host 下行（Clipboard→本机写入/Close）；ControlWrite::clipboard 便捷面；
  close 改 ctl_task.abort（读循环阻塞在 recv_control 不可协作取消）。
- 2026-09-15 crates/p2p-itest/tests/rd_clipboard_wave.rs：E2E 绿（viewer→host 写入、
  host 外部变更→viewer 两连发、回声抑制断言）；既有 input/video 波适配 with_config
  五参签名并全绿。
- 门禁：fmt/clippy -D warnings 零告警（四 crate + itest）。

## 里程碑状态
- M1：已收官合并（main @ dc870038）。
- M2：已收官合并（main @ 2a367e20）。
- M3：已收官合并（main @ cbc5072a）。
- M4：实现完成，待全量门禁绿后合并。
- M5：文件传输（浏览/上下传/进度/取消/续传/路径卫生落地）；见 plan.md。

## 依赖挂账
- p2p-service 服务开关（rd-host/rd-viewer）：wsm 波（feat/wsm-b2）合入后接线。
- p2p-authz 能力 key：M6 接线。
