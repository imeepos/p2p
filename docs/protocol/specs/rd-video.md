# /rd/video/1 规范

状态：draft；自 2026-09-15；归属 crates/rd-wire（implemented）；符合性：Core = 小端二进制
信封布局、codec 闭集、尺寸/rects/payload 上限、chunked 承载大帧；Extended = 无。
线协议总览见 docs/design/remote-desktop-plan.md §3.2。

## 1. 概览

屏幕画面通道：host→viewer 单向。每帧一个信封 + 编码载荷；载荷 > 1 MiB 时整体走
chunked 分块（复用底座 write_chunked/read_chunked，重组上限 64 MiB）。实时性优先：
视频通道不重传丢帧；viewer 以 keyframe 为同步点重绘。

## 2. 线格式

小端字节序，固定布局：

```
magic u8 = 0x52 | ver u8 = 1 | seq u32 | ts_ms u64 | w u16 | h u16 |
flags u8 | codec u8 | n_rects u16 | rects[n_rects × 8B] | payload_len u32 | payload
```

rect 8B = x u16 | y u16 | w u16 | h u16（帧内坐标，x+w ≤ w、y+h ≤ h）。

### 2.1 字段约束

| 字段 | 约束 |
|---|---|
| magic/ver | 0x52 / 1；不符即断流 |
| w,h | 1..=16384 |
| flags | bit0 = keyframe（同步点，viewer 重绘全帧） |
| codec | 0=raw_rgba，1=zlib_rgba；其余值断流（M6 增 lz4/h264 属加法升版本） |
| n_rects | ≤ 256 |
| payload_len | ≤ 256 MiB，且 MUST 与剩余字节数一致（截断/多余均断流） |

### 2.2 行为约定

- host 侧 fps 节流 + 阻塞即跳帧：只发最新帧；viewer 忽略乱序 seq 之前的帧。
- keyframe 每 N 帧至少一张（同步兜底，N 属实现策略）；viewer 未同步前丢弃 delta 帧。
