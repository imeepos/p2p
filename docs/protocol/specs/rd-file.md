# /rd/file/1 规范

状态：draft；自 2026-09-15；归属 crates/rd-wire（implemented）；符合性：Core = JSON 消息
全集、相对路径卫生（禁绝对/`..`/NUL）、数据块 ≤ 384 KiB（base64 落线）、传输状态机
（start→ack→data*→end | abort）；Extended = 无。线协议总览见
docs/design/remote-desktop-plan.md §3.3。

## 1. 概览

远程文件浏览与传输通道：双向 JSON 控制 + base64 数据块。目录浏览（list/stat/mkdir/rm）
与文件传输（upload=viewer→host、download=host→viewer）同通道；大文件按 384 KiB 原始
字节分块。host 侧 MUST 把 viewer 写入落到隔离目录（用户 `Downloads/RD/`），杜绝路径逃逸。

## 2. 线格式

帧封装同控制通道（varint 长度前缀 ≤ 1 MiB；384 KiB 原始 → base64 ≈ 512 KiB 可单片）。
消息 JSON UTF-8，tag = `type`。

### 2.1 消息全集（v1）

| type | 方向 | 字段 | 语义 |
|---|---|---|---|
| `fs_list` | viewer→host | path | 目录浏览请求 |
| `fs_list_ack` | host→viewer | path, entries[{name,kind,size,mtime}] | 条目应答（kind: file/dir/other） |
| `fs_stat` / `fs_stat_ack` | 双向 | path / path, entry? | 单条目 stat（entry 空 = 不存在） |
| `fs_mkdir` | viewer→host | path | 建目录 |
| `fs_rm` | viewer→host | path | 删除（host 实现可选回收站语义） |
| `xfer_start` | 双向 | id(uuid), path, size, direction(upload/download) | 传输登记（viewer 生成 id） |
| `xfer_ack` | 双向 | id, ok, reason?, offset | 接受/拒绝；offset = 续传起点 |
| `xfer_data` | 双向 | id, offset, data(base64) | 分块数据（≤ 384 KiB 原始） |
| `xfer_end` | 双向 | id, ok | 完成 |
| `xfer_abort` | 双向 | id | 单方取消 |
| `xfer_progress` | 双向 | id, done, total | 进度广播 |

### 2.2 约束

- 路径卫生（双方 MUST）：相对 POSIX、非空、≤ 4096 字符、无 NUL、无空段/`.`/`..` 段、
  不以 `/` 开头。违反即断流拒绝。
- `xfer_data.data` MUST 为合法 base64 且原始字节 ≤ 384 KiB；offset 与已收字节数
  不一致由宿主判重/续传策略处理（断点续传 M5）。
- 传输状态机：`xfer_start` → `xfer_ack`（唯一应答，其后才可有 data）→ data* → `xfer_end`；
  任意时刻 `xfer_abort` 终止；违反序 MUST 断流。
