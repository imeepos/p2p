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
| `fs_list` | viewer→host | path | 目录浏览请求（空串 = 根目录，M5 定） |
| `fs_list_ack` | host→viewer | path, entries[{name,kind,size,mtime}], error? | 条目应答（error 非空 = 失败） |
| `fs_stat` / `fs_stat_ack` | 双向 | path / path, entry?, error? | 单条目 stat（entry 空 = 不存在） |
| `fs_mkdir` | viewer→host | path | 建目录 |
| `fs_rm` | viewer→host | path | 删除（host 递归删目录；M5 不做回收站语义） |
| `fs_op_ack` | host→viewer | ok, reason? | 建目录/删除应答（M5 加法） |
| `xfer_start` | 双向 | id(uuid), path, size, direction(upload/download) | 传输登记（viewer 生成 id） |
| `xfer_ack` | 双向 | id, ok, reason?, offset | 接受/拒绝；offset = 续传起点 |
| `xfer_data` | 双向 | id, offset, data(base64) | 分块数据（≤ 384 KiB 原始） |
| `xfer_end` | 双向 | id, ok | 完成 |
| `xfer_abort` | 双向 | id | 单方取消 |
| `xfer_progress` | 双向 | id, done, total | 进度广播 |

### 2.2 约束

- 路径卫生（双方 MUST）：相对 POSIX、≤ 4096 字符、无 NUL、无空段/`.`/`..` 段、
  不以 `/` 开头；唯一例外是 `fs_list`/`fs_stat` 的空串 = 根目录浏览。违反即断流拒绝。
- host 侧落盘 MUST 限定在隔离根（默认 `$HOME/Downloads/RD`）：wire 卫生 +
  最深已存在祖先 canonicalize 前缀双检，防符号链接逃逸。
- `xfer_data.data` MUST 为合法 base64 且原始字节 ≤ 384 KiB；上传 offset MUST
  与当前游标严格一致（乱序/重复写即终止该传输并回 XferEnd{ok:false}）。
- 上传续传：host 对已存在且 ≤ 声明大小的部分文件续写，XferAck.offset 告知起点。
- 传输状态机：`xfer_start` → `xfer_ack`（唯一应答，其后才可有 data）→ data* → `xfer_end`；
  任意时刻 `xfer_abort` 终止；违反序 MUST 断流。
- 完传确认（M5 定）：viewer 收 host 的 `xfer_end` 才算上传完成（防读写竞态）。
- M5 限制：下载侧 host ack 后同步泵送，期间不读对端消息——下载中 abort 留 M6。
