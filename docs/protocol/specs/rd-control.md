# /rd/control/1 规范

状态：draft；自 2026-09-15；归属 crates/rd-wire（implemented）；符合性：Core = JSON 信封
`{"type":"<msg>"}`、消息全集与字段（下表）、解码端非法输入一律拒绝；Extended = 无。
线协议总览见 docs/design/remote-desktop-plan.md §3。

## 1. 概览

远程桌面会话的控制通道：握手（角色/能力协商）、输入事件、剪贴板同步、显示器拓扑、
质量协商、保活与显式关闭。双向；视频/音频/文件数据走独立通道（/rd/video/1、
/rd/file/1），控制通道只承载小载荷命令，避免命令饥饿。

## 2. 线格式

帧封装复用底座：varint 长度前缀，单帧 ≤ 1 MiB（docs/protocol/wire-format.md）。
控制消息为 JSON UTF-8，`serde` tag = `type` 字段；未知 type MUST 拒收（InvalidData 断流）。

### 2.1 消息全集（v1）

| type | 方向 | 字段 | 语义 |
|---|---|---|---|
| `hello` | viewer→host（host 首应） | v=1, role∈{host,viewer}, session_id(16 hex), caps{audio,file,clipboard} | 握手；v 未知回 hello_ack{ok:false} |
| `hello_ack` | 双向 | ok, reason?, session_id | 接受/拒绝；拒绝后会话关闭 |
| `input_mouse` | viewer→host | x,y(绝对坐标), buttons(bit0 左/1 右/2 中), wheel_dx/dy | 鼠标事件 |
| `input_key` | viewer→host | code(USB HID usage id u16), down, modifiers(0x1 shift/0x2 ctrl/0x4 alt/0x8 meta) | 键盘事件 |
| `input_key_reset` | viewer→host | — | 释放全部按键（失焦/断线时 host 必重置） |
| `clipboard` | 双向 | text(UTF-8 ≤ 8 MiB) | 文本剪贴板同步 |
| `display_list` | host→viewer | displays[{id,x,y,w,h,primary}], current | 显示器拓扑 |
| `display_select` | viewer→host | id | 切换捕获目标（M6 多屏） |
| `quality` | viewer→host | fps, scale, codec | 质量协商请求 |
| `quality_ack` | host→viewer | fps, scale, codec | 采纳档位 |
| `heartbeat` | 双向 | seq | 保活；5s 无心跳判活 |
| `close` | 双向 | reason | 显式会话关闭 |

### 2.2 约束

- `hello.v` MUST 为 1；`session_id` MUST 为 16 hex（访问侧生成，两侧日志同源）。
- `clipboard.text` UTF-8 字节数 MUST ≤ 8 MiB，超限拒收。
- 解码端对任何字段越界/类型错误 MUST 以 InvalidData 断流，不猜测不降级。
- 会话存活判据：heartbeat 双向 5s 窗口；超时由宿主按策略关闭并审计。
- 输入注入（M3 实现）：
  - `input_key.code` 为 USB HID 键盘 usage id；宿主维护权威修饰键按住集合，
    事件 flags 反映宿主态（不采信 viewer 上报态，防漂移）。
  - 会话断开（显式 close/超时/流错）宿主 MUST 调 `input_key_reset` 语义
    （释放全部按键），防远程卡键。
  - 未映射键码宿主 MUST 丢弃并告警（不清流，输入错误不中断会话）。
