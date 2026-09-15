# 远程桌面控制 over p2p（remote-desktop-plan）v1

状态：定稿（2026-09-15）；波次账本 `.orchestrator/2026-09-15-remote-desktop/`。

## 1. 目标与验收口径

在 p2p-base 底座之上实现商用级远程桌面控制，功能面以 RustDesk 核心功能为基线：

| 功能 | 验收口径（商用程度） |
|---|---|
| 屏幕画面实时传输 | 10–30 FPS 可调、按需分辨率/缩放、首帧 ≤ 1s、丢帧有界（视频通道无重传） |
| 鼠标控制 | 移动/按键/滚轮/绝对坐标；双屏缩放坐标映射；悬停/离开事件 |
| 键盘控制 | 全键码透传 + 修饰键状态机；组合键（Cmd/Ctrl+…）正确送达 |
| 剪贴板同步 | 双向文本剪贴板（M4）；HTML/富文本与文件列表（M6 增强） |
| 文件传输 | 远程目录浏览、上传/下载、多文件、进度与取消、断点续传（M5） |
| 会话管理 | 发起/接收/拒绝/断开；临时口令与白名单准入；会话审计日志 |
| 安全 | 复用 p2p 身份互认（PeerId 即密钥材料）；接入 p2p-authz 权限模型与服务总控开关 |
| 商用健壮性 | 断线自动重连、质量自适应（fps/编码档位）、多显示器、权限缺失可观测提示、无 panic 路径 |

明确不做（后续波可另立目标）：音频通话（M7 候选）、多对一会话中继编排、移动端客户端。

## 2. 架构

### 2.1 角色模型

同一二进制（p2p 节点 + GUI）双角色：

- **Host（被控端）**：屏幕采集、输入注入、剪贴板宿主、文件宿主。一个 host 会话可被一个 viewer 接入。
- **Viewer（控制端）**：画面渲染、本地输入采集、剪贴板同步发起方、文件操作发起方。

角色在握手帧中协商：发起连接的一方默认为 viewer；host 侧显式开启「允许远程控制」服务后
监听入站。方向约定：视频/音频通道只 host→viewer；控制通道双向。

### 2.2 通道划分（1 逻辑流 = 1 p2p 流）

复用底座「一流一事务」惯例，每会话按需开 2–4 条逻辑流：

| 协议 ID | 方向 | 载荷 | 语义 |
|---|---|---|---|
| `/rd/control/1` | 双向 | JSON 控制帧 | 握手、会话参数、输入事件、剪贴板、显示信息、质量协商、心跳、关闭 |
| `/rd/video/1` | host→viewer | 二进制帧 | 屏幕画面：帧信封 + 编码载荷（chunked 承载大帧） |
| `/rd/audio/1` | host→viewer | 二进制帧 | 音频（M7；信道先注册后实现） |
| `/rd/file/1` | 双向 | JSON 控制 + 二进制数据 | 远程文件浏览与传输 |

控制通道承担全部小载荷与命令；大载荷（画面/文件数据）走专用通道，避免命令饥饿。
视频通道不重传丢帧（实时性优先）；控制通道帧序严格（输入必须有序送达）。
同一 Peer 同时只允许一个活跃会话，视频流按 PeerId 与 control 会话隐式绑定
（不做显式 bind 帧；多会话并发在 M6 多显示器波再演进）。

### 2.3 Crate 地图（crates/rd-*）

| crate | 职责 | 平台 |
|---|---|---|
| `rd-wire` | 全部线协议消息模型 + 编解码 + 帧封装/分块 I/O（纯逻辑） | 全平台 |
| `rd-capture` | 屏幕采集抽象：CaptureSource trait + 确定性合成源（E2E）；`sck` feature 启用 ScreenCaptureKit 真实源（macOS 13+，Swift 运行时） | macOS（后续多平台） |
| `rd-input` | 输入注入：CGEvent 鼠标/键盘；修饰键状态机；Accessibility 授权探测 | macOS |
| `rd-clipboard` | 剪贴板读写与变更订阅（arboard 或 NSPasteboard） | macOS（arboard 跨平台） |
| `rd-fs` | 远程文件系统：目录浏览、stat、传输分块、续传索引 | 全平台 |
| `rd-host` | host 侧会话装配：capture+clipboard+fs 接线、准入审批、审计 | 全平台 |
| `rd-viewer` | viewer 侧会话装配：解码渲染管线、输入采集接线 | 全平台 |
| `rd-service` | GUI 命令面（src-tauri 装配 + p2pctl rd 子命令） | macOS |

依赖方向：rd-* → p2p facade；rd-host/viewer → rd-{capture,input,clipboard,fs,wire}。

### 2.4 与既有底座的关系

- 传输/身份/加密/发现/穿透/中继：全部复用 p2p facade（`Node::new_stream` / `handle_protocol`）。
- 会话准入：白名单走 p2p-authz 好友权限模型（rd 能力 key 登记后接 PEP）；服务总控开关接
  p2p-service 注册表（wsm 波合入后接线，见依赖节）。
- 审计：会话建立/断开/输入/文件事件写入 p2p-log 域（host 侧强制，viewer 侧可选）。

## 3. 线协议

### 3.0 帧封装

- 小帧（≤ 1 MiB）：复用底座 varint 长度前缀帧（`p2p_protocol::write_frame/read_frame`）。
- 大载荷（画面、文件数据）：复用 `write_chunked/read_chunked`（CHUNK_DATA_SIZE 分块）。
- 控制消息 JSON UTF-8，`serde` tag = `type` 字段，未知 type 拒收并回 `ProtocolError`。

### 3.1 /rd/control/1 控制消息

信封：`{"type":"<Msg>", ...字段}`。消息全集（v1）：

| type | 方向 | 字段 | 语义 |
|---|---|---|---|
| `hello` | 双向 | v, role, session_id, caps, screen_w/h, dpi | 握手；viewer 先发 |
| `hello_ack` | 双向 | ok, reason, session_id, displays[], fps_max | 接受/拒绝；拒绝带结构化 reason |
| `input_mouse` | viewer→host | x, y, buttons, wheel_dx/dy | 绝对坐标 + 按键掩码 + 滚轮增量 |
| `input_key` | viewer→host | code, down, modifiers | 键码（USB HID 码），down=true 按下 |
| `input_key_reset` | viewer→host | — | 释放全部按键（失焦/断线时 host 必重置） |
| `clipboard` | 双向 | text | 文本剪贴板（≤ 8 MiB） |
| `display_list` | host→viewer | displays[] (id,x,y,w,h,primary), current | 显示器拓扑（M6 多屏） |
| `display_select` | viewer→host | id | 切换捕获目标（M6） |
| `quality` | viewer→host | fps, scale, codec | 质量协商（M6 自适应） |
| `quality_ack` | host→viewer | fps, scale, codec | 采纳档位 |
| `heartbeat` | 双向 | seq | 保活；5s 无心跳判活 |
| `close` | 双向 | reason | 会话关闭（显式） |

字段约束：`role ∈ {host, viewer}`；`v` 固定 1；未知版本 hello 回 `hello_ack{ok:false}`。

### 3.2 /rd/video/1 视频帧

二进制信封（手写编解码，紧凑优先，小端）：

```
magic u8=0x52 | ver u8=1 | seq u32 | ts_ms u64 | w u16 | h u16 |
flags u8 (bit0 keyframe) | codec u8 | n_rects u16 | rects[ n_rects × (x u16,y u16,w u16,h u16) ] |
payload_len u32 | payload[payload_len]
```

- `codec ∈ {0 raw_rgba, 1 zlib_rgba}`（M1 起）；M6+ 增 `{2 lz4, 3 h264}`（硬件编码）。
- `flags.keyframe`：关键帧（全屏 rect 或全量 payload），viewer 以此为同步点。
- payload > 1 MiB 时信封整体走 chunked；信封头部随首个 chunk 携带。
- 丢帧策略：host 只发最新帧（fps 节流 + 阻塞即跳帧），viewer 以 keyframe 重同步。

### 3.3 /rd/file/1 文件传输

JSON 控制消息（双向）+ 数据块（viewer↔host 双向，按传输方向）：

| type | 方向 | 字段 | 语义 |
|---|---|---|---|
| `fs_list` | viewer→host | path | 目录浏览 |
| `fs_list_ack` | host→viewer | entries[] (name, kind, size, mtime), path | 条目清单 |
| `fs_stat` / `fs_stat_ack` | 双向 | path / entry | 单条目 |
| `fs_mkdir` / `fs_rm` | viewer→host | path | 建目录/删除（回收站语义 M6） |
| `xfer_start` | 双向 | id, path, size, direction | 传输登记（upload=viewer→host） |
| `xfer_ack` | 双向 | id, ok, reason, offset | 接受（可带续传 offset） |
| `xfer_data` | 双向 | id, offset, data_b64 | 分块数据（≤ 512 KiB/块） |
| `xfer_end` / `xfer_abort` | 双向 | id, ok | 完成/取消 |
| `xfer_progress` | 双向 | id, done, total | 进度广播 |

- 传输 id 为 viewer 生成 uuid（会话内唯一）；host 侧默认把 upload 落到用户目录下
  `Downloads/RD/` 隔离区（防路径穿越，M5 起强制）。
- 路径卫生：拒绝绝对路径与 `..` 逃逸（`rd-fs` 纯函数，单测覆盖）。

### 3.4 /rd/audio/1（M7 候选）

预留信道，注册表先登记 `planned`，实现随音频波落地（AAC over chunked）。

## 4. 安全与权限

1. 传输安全：p2p 底座 QUIC/TLS 或 TCP/Noise 已做身份互认与全量加密，rd 层不重复加密。
2. 会话准入两闸：
   - 静态闸：`p2p-authz` 权限模型（能力 key `rd.control`/`rd.file`/`rd.clipboard`，按好友绑定）。
   - 动态闸：host 侧会话审批（GUI 弹窗接受/拒绝，或一次性临时口令，RustDesk 同款模式）。
3. 服务总控：p2p-service 注册表增 `rd-host`（监听）与 `rd-viewer`（拨号）开关，默认关。
4. 输入注入与屏幕采集分别要求 macOS 辅助功能 / 屏幕录制授权；权限缺失时握手前显式
   拒绝并给可观测错误（沿用 GUI 截图权限先例 CAPTURE_PERMISSION_DENIED 形态）。
5. 审计：host 侧强制记录 session open/close、peer、时长、文件操作；落 p2p-log 滚动域。

## 5. GUI 面（apps/gui views/remote-desktop）

- 入口：remote-access 页增「远程桌面」分区（隧道卡并排，互不干扰）。
- Host 态：服务开关、会话审批队列（接受/拒绝 + 临时口令生成）、在线会话列表。
- Viewer 态：连接对话框（peer 选择/口令）、会话窗口（画面渲染 canvas、工具栏：
  缩放/质量/剪贴板/文件/断开）。
- src-tauri 命令面（generate_handler 登记）：`rd_host_start/stop`、`rd_viewer_connect/disconnect`、
  `rd_frame_next`（事件流订阅）、`rd_clipboard_push/pull`、`rd_fs_list/transfer`。
- CLI 对等（p2pctl rd 子命令）与 cli-parity.tsv 同步登记（M6 收口）。

## 6. 里程碑与验收

| 里程碑 | 内容 | 验收 |
|---|---|---|
| M1（本轮） | 设计定稿 + `crates/rd-wire`（全部消息模型/编解码/帧 I/O + 单测）+ 协议注册（registry.toml/specs/wire-protocol.md） | make check 绿；rd-wire 单测覆盖编解码往返/非法输入拒绝/分块边界 |
| M2 | video 全链：rd-capture（trait+合成源+SCK feature 门控）+ rd-host/rd-viewer 会话 + 双节点 E2E（raw/zlib 双编码、keyframe 同步、重复拨号拒绝、close 清理） | 3 项 E2E 绿（crates/p2p-itest/tests/rd_video_wave.rs）；SCK 真实采集待带授权真机验证（本机无 Swift 运行时） |
| M3 | 输入注入（鼠标/键盘/修饰键）+ input 通道 | 真机远程可操作；断线按键重置；Accessibility 缺失显式报错 |
| M4 | 剪贴板双向同步 | 文本剪贴板两端一致；变更订阅驱动 |
| M5 | 文件传输（浏览/上下传/进度/取消/续传/路径卫生） | 单测 + 双节点 E2E 大文件校验和一致 |
| M6 | 商用收口：多显示器/质量自适应/重连/审批 GUI/审计/authz+服务开关接线/CLI 对等 | 全量 make check；真机走查报告 |
| M7（可选） | 音频 + 硬件编码（H.264） | 独立验收 |

每里程碑独立收官：worktree 反向同步 → 合并 → 清分支 → 账本更新（AGENTS.md 收尾四步）。

## 7. 依赖与协调

- **wsm 波（服务总控，session-cd1cdce3 在飞，feat/wsm-b2）**：rd-host/rd-viewer 服务开关
  在 p2p-service 注册表登记，wsm 合入 main 后接线；本轮不触碰 p2p-service。
- **tunnel 波**：已收官；remote-access 页隧道卡为并行面，rd 页独立分区不重叠。
- **协议注册门禁**：registry.toml 四向核对，本轮新增 4 个协议 ID 须同步 specs 与
  wire-protocol.md §3.2（每协议一条）。

## 8. 冻结接口（v1，改动需评审）

- 协议 ID：`/rd/control/1`、`/rd/video/1`、`/rd/audio/1`（planned）、`/rd/file/1`。
- 控制消息 type 全集与字段（§3.1）；视频信封布局（§3.2）；文件消息全集（§3.3）。
- 编解码入口：`rd_wire::control::{encode, decode}`、`rd_wire::video::{encode_frame, decode_frame}`、
  `rd_wire::file::{encode, decode}`；I/O：`rd_wire::io::{send_control, recv_control, send_large, recv_large}`。
