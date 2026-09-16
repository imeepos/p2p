# /ftp/ctrl/1 与 /ftp/data/1 规范

状态：stable；自 2026-09-16；归属 crates/p2p-ftp；符合性：Core 项=全部章节（互操作必需行为契约）

## 1. 概览

- 解决什么问题：在 p2p 底座上提供 FTP 语义的文件传输（浏览目录、上传、下载、
  改名、删除），复用经典 FTP 的控制/数据双通道模型。
- 底座映射：p2p 无 IP 寻址，经典 FTP 的 PORT/PASV「第三连接」不可用；
  改由服务端签发的一次性传输令牌承接——控制应答 `150 ok token=<hex>` 发牌，
  数据流首帧出示令牌，登记簿校验令牌与来源节点后执行传输。
- 控制通道 `/ftp/ctrl/1`：一帧一行 UTF-8；数据通道 `/ftp/data/1`：首帧 =
  操作码 + 令牌，其后原始字节流（帧封装仅用于首帧，文件体不分帧直通）。
- 底座只做路由（design §9）：本协议为纯业务层，宿主经 `p2p_ftp::serve()`
  装配两个 handler，无特权差别。

## 2. 线格式

两协议首帧均为协议 ID 帧（wire-protocol.md §4），之后：

### 2.1 控制通道（/ftp/ctrl/1）

每帧 = 一行 UTF-8（`varint(len) + payload`，payload ≤ 1 MiB）：

- 服务端连上即发问候帧 `220 p2p-ftp ready`。
- 客户端命令帧：`<VERB> [arg]`，如 `RETR /docs/a.txt`、`LIST`（缺省列 cwd）。
- 服务端应答帧：`<3位码> <text>` 单帧单条，禁止多行。

命令集（RFC 959 子集）：

| 命令 | 参数 | 成功码 | 说明 |
|---|---|---|---|
| USER / PASS | 账号 / 口令 | 331 → 230 | 登录序列；PASS 前必须 USER（否则 503） |
| SYST / FEAT / NOOP | 无 | 215 / 211 / 200 | 探测与保活 |
| TYPE | 类型 | 200 | 恒为二进制语义，参数原样接受 |
| QUIT | 无 | 221 | 服务端随后关流 |
| PWD | 无 | 257 | 应答含引号路径 `"<cwd>"` |
| CWD / CDUP | 路径 | 250 | 目标必须是目录（否则 550） |
| MKD | 路径 | 257 | |
| RMD / DELE | 路径 | 250 | 目录须为空 |
| RNFR / RNTO | 路径 | 350 → 250 | RNTO 前必须 RNFR（否则 503） |
| SIZE | 文件路径 | 213 | 应答 text 为十进制字节数 |
| LIST / NLST | 可选路径 | 150 → 226 | 走数据通道 |
| RETR / STOR / APPE | 路径 | 150 → 226 | 走数据通道；APPE 追加，STOR 截断 |

未识别命令回 500；语法缺参折叠为 Unknown 回 500；时序错乱回 503；
未登录（白名单外）回 530。

### 2.2 数据通道（/ftp/data/1）

首业务帧 payload：`1 字节操作码 + 32 字节令牌`。操作码：
`0x01`=GET（服务端→客户端）、`0x02`=PUT（客户端→服务端，APPE 复用）、
`0x03`=LIST 明细、`0x04`=NLST 名字。

之后为原始字节流，不分帧：

- GET/LIST/NLST：服务端写完整 payload 后半关流（EOF）。
- PUT：客户端写完后半关流；服务端读到 EOF 即落盘完成。STOR 服务端走
  HiddenStores（ProFTPD 同名机制）：先写同目录隐藏临时文件
  `.<name>.p2p-ftp-partial`，成功后原子 rename 到目标，任何失败（超限/断流/
  rename 失败）自动清临时文件——目标路径永不出现半截文件；APPE 直写目标
  （追加语义=断点续传友好）。

LIST 明细行：`<d|f>\t<size>\t<mtime>\t<name>\n`（类型 d=目录 f=文件，
mtime 为 Unix 秒；名字含空格安全，坏行由客户端跳过）。
NLST：每行一个名字。

### 2.3 传输令牌

- 签发：控制应答 `150 ok token=<64位hex>`；token 为 256 位 CSPRNG。
- 兑付：数据流首帧 token 必须等于未过期令牌且来源节点 == 签发节点；
  对端不符不动令牌（留给合法节点），过期即作废移除。
- TTL：默认 60s（签发起算），登记簿在下次签发时惰性清理。

## 3. 时序与状态机

单控制连接内传输串行（与经典 FTP 控制连接同义）：

    RETR: C→S RETR path | S→C 150 token | C 开数据流(0x01+token)
          S 推字节至 EOF | S→C 226 ok n=<字节数>（失败 426/451/550）
    STOR: C→S STOR path | S→C 150 token | C 开数据流(0x02+token)
          C 推字节后半关流 | S→C 226（超限 552/父目录缺失 550）

会话状态（登录态/虚拟 cwd/RNFR 暂存）仅存于本连接，断开即丢弃。
虚拟路径以 `/` 起始，`..` 越出根一律 550（`normalize` 先拒）。

## 4. 应答码

沿用 RFC 959 语义：220 问候；331/230 登录；215/211/200 探测；221 退出；
250/257/213 文件操作；350 更名就绪；150 传输发起；226 完成；
426 数据通道失败/超时；451 服务端本地错误；500 未知命令；501 语法；
503 时序；530 未登录/登录失败；550 不存在/越狱/形态不符；552 超限。

## 5. 安全

- 鉴权接缝：`Authenticator::login(peer, user, pass) -> bool` 单点判定，
  OpenAuth（全放行，仅测试/本机）与 StaticAuth（精确匹配）随 crate 提供；
  p2p-authz 权限模型收编属宿主装配侧工作。
- 路径监狱（LocalFs 后端）：组件层过滤 `..`；已存在路径 canonicalize 后
  必须仍在根内（反符号链接逃逸）；写路径校验父目录在根内。
  会话层 `normalize` 先拒 `..` 越根（双闸纵深）。
- 上传限额：`FtpConfig.max_upload_bytes`（默认 256 MiB）逐片累计，
  超限即断流回 552，禁止静默截断；HiddenStores 保证失败零残留。
- 列表上限：`FtpConfig.max_list_entries`（默认 10 000），超限显式报错，
  防超大目录拖垮内存/带宽（FTP 无分页标准的社区通行防御位）。
- 裸流拒绝：控制/数据 handler 均要求对端身份（swarm 安全握手互认），
  无身份上下文的裸流一律 PermissionDenied（令牌绑定签发方不可绕过）。

## 6. 常量

| 常量 | 值 | 出处 |
|---|---|---|
| 控制协议 ID | `/ftp/ctrl/1` | crates/p2p-ftp/src/lib.rs |
| 数据协议 ID | `/ftp/data/1` | crates/p2p-ftp/src/lib.rs |
| 令牌长度 | 32 字节 | crates/p2p-ftp/src/wire.rs TOKEN_LEN |
| 单帧上限 | 1 MiB | 底座 MAX_FRAME_SIZE（控制帧/数据首帧适用） |
| 默认上传上限 | 256 MiB | crates/p2p-ftp/src/server.rs FtpConfig |
| 默认令牌 TTL / 传输时限 | 60s / 300s | crates/p2p-ftp/src/server.rs FtpConfig |

验收：crates/p2p-ftp/tests/roundtrip.rs 双节点真实回环（登录→建目录→
2 MiB 上传下载→追加/改名→删除全链、鉴权门、路径监狱、上传限额）。
