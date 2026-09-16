# /vdrive/fs/1 规范

状态：stable；自 2026-09-16；归属 crates/p2p-vdrive；符合性：Core 项=全部章节（互操作必需行为契约）

## 1. 概览

- 解决什么问题：在 p2p 底座上提供「网络硬盘」——对端把本地目录发布为
  全网可见的盘，本端把远端目录**直接挂载**成 OS 文件系统（macOS Finder/
  mount_webdav、Linux davfs2、Windows net use）。
- 组成两层：
  - `/vdrive/fs/1`：一请求一流的文件系统操作面（本规范）；
  - 挂载桥（非 p2p 协议，仅本机回环 HTTP）：把操作面翻译成 WebDAV
    Class 1，OS 原生客户端零驱动直接挂载（specs 不登记，见 §6）。
- 底座只做路由（design §9）：纯业务层；传输加密与对端身份互认由底座
  安全握手承担，本协议面不再自设账号体系。
- 与 /ftp 的关系：ftp 面向「人传文件」（命令行/双通道令牌），vdrive
  面向「程序挂盘」（偏移读写 + WebDAV 语义），并存不复用协议。

## 2. 线格式

首帧为协议 ID 帧（wire-protocol.md §4），其后每请求占一根流：

### 2.1 请求帧

一帧 JSON（`varint(len) + payload`，≤ 1 MiB），serde 内标签：

```json
{"op":"ping"}
{"op":"statfs"}
{"op":"stat","path":"/a/b.txt"}
{"op":"list","path":"/docs"}
{"op":"mkdir","path":"/docs"}
{"op":"rmdir","path":"/docs"}
{"op":"unlink","path":"/a/b.txt"}
{"op":"rename","from":"/a","to":"/b"}
{"op":"truncate","path":"/a","size":1024}
{"op":"create","path":"/a/new.txt"}
{"op":"read","path":"/a","offset":0,"len":524288}
{"op":"write","path":"/a","offset":0}
```

- 路径一律服务端虚拟路径：`/` 起始，根 = 后端根目录；
- write 请求帧之后必须随一帧原始数据（长度 ≤ MAX_CHUNK）。

### 2.2 应答帧

一帧 JSON：

- 成功：`{"ok":true,"data":<载荷>}`；data 按 op：
  - ping/mkdir/rmdir/unlink/rename/truncate → `null`
  - statfs → `{"total_bytes":u64,"free_bytes":u64}`（0 = 不报告）
  - stat/create → Entry `{"name","kind":"file|dir","size","mtime","ctime"}`
    （mtime/ctime 为 unix 秒）
  - list → `[Entry, ...]`
  - write → `{"written":u64}`
  - read → `null`，**应答帧之后随一帧原始数据**（实际读得字节，可为
    短读/0，EOF 用短读表达）
- 失败：`{"ok":false,"err":{"kind":<ErrorKind>,"msg":"..."}}`。

| ErrorKind | 语义 |
|---|---|
| not_found | 目标或父目录不存在 |
| already_exists | 目标已存在 |
| not_empty | 目录非空（rmdir） |
| not_dir | 对目录执行文件操作（读目录体/删目录体） |
| permission_denied | 策略拒绝（含只读模式） |
| invalid_path | 路径越出根 / 形态非法 |
| too_large | read len 超上限 / write 数据超上限 |
| unsupported | 后端不支持该操作 |
| io | 其他后端 IO 错误 |

## 3. 语义约定

- 一请求一流：无会话态，连接复用由底座连接池承担；流上请求帧即
  EOF 前唯一一帧，服务端应答后关流。
- read len 为 0 或 > MAX_CHUNK：服务端直接 too_large 拒绝（客户端
  分块循环拉取；短读即推进游标）。
- write 为定位写：文件不存在则创建（不截断，长尾覆盖走 truncate）；
  返回 written 恒等于数据帧长度，写不满即报错。
- rename 为 POSIX 语义：目标存在则覆盖（目录覆盖要求为空）。
- create 为创建或截断（PUT 新建与覆盖共用），父目录缺失 not_found。
- statfs 允许后端不报告容量（0/0），挂载桥据此省略配额属性；LocalFs 经
  libc::statvfs 报告宿主真容量（f_frsize × f_blocks / f_bavail）。

## 4. 安全

- 身份：仅受理已互认身份的入站流，裸流一律 PermissionDenied
  （VDriveServer::handle 显式拒绝，禁静默服务）。
- 访问策略接缝：`AccessPolicy::allow(peer, OpClass)` 按读/写两类裁决；
  宿主可接 authz（capability key 登记随服务总控波），`--read-only`
  为内置只读策略。
- 路径监狱（LocalFs）：组件层折叠 `..`（越根即 invalid_path）+
  目标/最近存在祖先 canonicalize 前缀校验（反符号链接逃逸）双闸；
  根目录 rmdir 显式拒绝。
- 数据上限：单数据帧 ≤ MAX_CHUNK（512 KiB，帧上限 1 MiB 内留余量）。

## 5. 挂载桥（本机回环 WebDAV）

- 绑定 127.0.0.1（默认随机端口），网络边界在底座、本机边界在回环，
  桥不增设鉴权面。
- 方法面：OPTIONS / PROPFIND / GET / HEAD / PUT / MKCOL / DELETE /
  MOVE / COPY / PROPPATCH(形答 207) / LOCK(假持锁) / UNLOCK。
- 兼容取舍：PROPFIND 忽略请求体恒按超集属性应答；Depth: infinity
  显式 400（RFC 4918 §9.1.1 允许；社区先例：Apache mod_dav 的
  DavDepthInfinity 指令默认 off 防 DoS，与本桥同取向）；collection
  条目附 RFC 4331 quota-used/available-bytes（容量未知时省略，
  Finder/Windows 据此显示剩余空间）；DELETE/COPY 为 Depth: infinity
  递归语义；mtime 以后端系统语义为准，PROPPATCH 不落盘。
- 流式：GET 下行与 PUT 上行逐块搬运（512 KiB chunk），桥内存占用
  恒定，不整文件驻留。
- macOS：`mount_webdav http://127.0.0.1:<port>/ /Volumes/<name>`；
  p2pctl vdrive mount --mount 自动执行并在退出时 diskutil unmount。

## 6. 常量

| 常量 | 值 | 出处 |
|---|---|---|
| 协议 ID | `/vdrive/fs/1` | crates/p2p-vdrive/src/wire.rs |
| 单 chunk 上限 | 512 KiB | crates/p2p-vdrive/src/wire.rs MAX_CHUNK |
| 单帧上限 | 1 MiB | 底座 MAX_FRAME_SIZE |
| 桥监听 | 127.0.0.1:0（默认随机） | crates/p2p-vdrive/src/bridge.rs MountConfig |
| 请求头解析限时 | 10 s | crates/p2p-vdrive/src/http/mod.rs HEAD_TIMEOUT |

- 大文件下行：GET 经 FsBackend::open_reader 一次传输一次句柄
  （LocalFs 持 tokio::fs::File；远端客户端缺省 chunk 泵），禁逐块重开
  （SFTP SftpInputStreamAsync 同款先例）。

验收：crates/p2p-vdrive 单测（wire/监狱/日期/XML/HTTP 体/statfs/流式读）+
crates/p2p-itest/tests/vdrive_wave.rs 双节点全链（协议操作面 +
真 TCP 回环 WebDAV 方法面 + 越狱拒绝）；真机挂载冒烟
scripts/ops/vdrive-mount-smoke.sh（mount_webdav 实挂实测）。
