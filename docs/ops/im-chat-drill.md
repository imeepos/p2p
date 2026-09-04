# IM 聊天真机演练清单（批次一全链）

> 基线 main @ f732bc4（2026-09-03）| 契约：docs/design/im-chat-design.md v1 +
> gui-contract.md §12 v7 + wire-protocol.md §8.1（/im/chat/1）。
> 定位：人工里程碑，不入账本；机械验收以 T33 验收命令
> `cargo test -p p2p-itest --test chat_e2e && make check` 全绿为准（plan §7）。
> 本清单为双节点真机手工演练步骤；真机执行由协调者安排，本单只交付文档。
> 2026-09-04 增补：2.10 回复消息场景（IM-T46A replyTo 契约落地；步骤 1 的 GUI
> 交互入口待 IM-T46B 合入后执行，后端链路可先以 chat_e2e 引用场景机械验证）。

## 1. 演练前置

两个聊天节点（互为好友）：推荐 **Tauri GUI 双开**（同一台机器跑两个实例，
dataDir 必须隔离，见下）；或 **一个 Tauri + 一个 p2p-cli 辅助节点**
（p2p-cli `node` 子命令只装配 echo handler，无法收发 /im/chat/1，
仅能验证拨号可达，聊天收发仍须在 GUI 两端做——见 §2.0 校准项 1）。

| 节点 | 形态 | 身份/数据 | 日志位置 |
|---|---|---|---|
| 节点 A | Tauri GUI | `<app_data_dir>/p2p-data`（chat 数据在 `<data_dir>/chat`） | GUI 控制台 + `<app_log_dir>/p2p-console.log`（p2p-log 滚动文件） |
| 节点 B | Tauri GUI（双开，改 dataDir）或 p2p-cli node | 同上（双开时 dataDir 须不同，避免同身份互拒） | p2p-cli：stdout 打印 `peer_id`/`listen_addrs`，日志走 stderr |

双开 dataDir 隔离：Tauri 侧在设置页把 dataDir 改成独立路径（如 `~/p2p-drill-b/p2p-data`）
并重启节点；双开共用同一 dataDir 会因身份相同触发 friend_add 的 SelfPeer 拒绝。

peerId + addr 来源（加好友前先取到）：
- GUI：节点状态页显示 peer_id / listen_addrs（node_status 返回）；
- p2p-cli：`p2p-cli node --data <dir>` 启动时 stdout 打印
  `peer_id=` 与 `listen_addrs=[...]`（tcp 地址形如 `/ip4/127.0.0.1/tcp/<port>`）；
- discover：`p2p-cli discover` 列出发现节点（同机 mDNS 开启时可见）。

加好友（两端互加，命令面 gui-contract §12.1）：
`chat_friend_add(peerId, nickname, addrs)`——addr 填对端 TCP 监听地址；
peerId 必须为合法 base58 且 ≠ 本机，否则 Err（可读中文）。

## 2. 逐项演练（每步含预期与失败排查）

### 2.0 校准项（如实登记）

1. **p2p-cli 无 chat handler（T32 现状）**：`p2p-cli node` 仅装配 echo，
   对端 GUI 发 /im/chat/1 流会 UnsupportedProtocol 断流。演练主路径用双 GUI；
   p2p-cli 只承担"辅助拨号可达性"观测。

### 2.1 文本互通

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 侧聊天页选好友 B，发送 "hello" | 消息气泡出现，状态 sent（短瞬）后 delivered |
| 2 | B 侧聊天页 | 实时收到 "hello"（chat_message 事件），状态 delivered |

失败排查：
- B 未收到 → A 侧日志查 outbox 是否 flush 失败（"对端未连接，消息保持 pending"）；
  确认好友 addr 是 B 当前监听地址（B 重启后地址变化需重加好友）。
- 状态停在 pending/sent 不翻 delivered → 查 p2p-console.log 中 /im/chat/1 入站告警
  （帧校验失败会断流留 warn）。

### 2.2 emoji 消息

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 发送含 emoji 的文本（如 "👍 收到"） | emoji 原样显示在 B 侧（emoji 即 unicode 文本，走 text kind） |
| 2 | B 回一条含 emoji 文本 | A 侧原样显示 |

失败排查：乱码/丢失 → 文本为 UTF-8，确认前端字体与输入法正常；协议侧无特殊编码。

### 2.3 图片上传与预览

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 选一张 png/jpeg 图片发送 | 发送成功，B 收到图片消息 |
| 2 | B 点击图片 | 内联预览显示原图（chat_media_file 返回 asset URL，经 assetProtocol 加载） |

失败排查：
- 预览空白 → 检查 assetProtocol scope 是否含 chat/media 目录（src-tauri 已配置，
  若改动过 tauri.conf.json 需核对）；B 侧 `<data_dir>/chat/media/<peerId>/` 应有文件。
- 上传报错 → 图片 mime 必须在 image 白名单（png/jpeg/gif/webp），其他 mime 会 Err。

### 2.4 音频上传播放

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 发送音频（mpeg/wav/ogg/m4a/mp4） | 送达，B 侧显示音频消息 |
| 2 | B 点击播放 | 可播放（asset URL 加载） |

失败排查：mime 非白名单（如 audio/flac）→ 发送即 Err，属预期拒绝；
播放失败 → 查文件落盘与 asset scope（同 2.3）。

### 2.5 视频上传播放

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 发送 mp4/webm/mov 视频 | 送达，B 侧显示视频消息 |
| 2 | B 点击播放 | 可播放 |

失败排查：同 2.4；视频体积接近 64MiB 上限时发送前被拒（MediaTooLarge），
演练用 ≤10MiB 样例。

### 2.6 任意文件下载

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 发送任意类型文件（text/plain、application/* 等，kind=file） | 送达，B 侧显示文件名/大小 |
| 2 | B 点击下载/保存 | 文件保存成功，内容与 A 侧原文件一致 |

失败排查：file kind 不限 mime（白名单其余归 file）；字节不一致 → 链路分片重组
问题，查两端日志媒体帧告警。

### 2.7 发送状态流转（sent → delivered）

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | 两端在线，A 发消息 | 状态：pending → sent → delivered（chat_status 事件驱动 UI） |
| 2 | 断网（或停 B）后 A 发消息 | 停在 pending，UI 可见未送达标记 |

失败排查：不翻 delivered → 对端 ACK 未回（查对端日志）；UI 状态不刷新 → 查
chat_status 事件转发（src-tauri events 通道）。

### 2.8 离线消息（收方下线 → 上线自动收到）

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | B 下线（关节点） | A 侧好友 B 离线 |
| 2 | A 给 B 发消息 | 送达失败但消息保留（pending，outbox 落盘） |
| 3 | B 重新上线（同 dataDir 启动，身份不变） | A outbox 自动 flush → delivered；B 上线后收到全部离线消息 |
| 4 | 重启后核对好友簿 | 好友仍在（friends.json 持久化），A 侧历史含离线消息 |

失败排查：
- B 重启后 A 仍 pending → B 监听地址变化，A 地址簿仍是旧地址：
  在 A 侧重新 chat_friend_add（新 addr）或 B 用固定监听端口重启。
- flush 未触发 → 查 A 日志 PeerConnected 事件与 outbox 重发告警。

### 2.9 历史分页回读

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A/B 互发 ≥5 条消息（含重启前后） | 双方历史均完整 |
| 2 | A 重启（同 dataDir）后打开会话 | 历史全部回读（messages/<peer>.jsonl 持久化） |
| 3 | 上滑加载更多 | 按 beforeId 游标分页返回更早消息，无重复无遗漏 |

失败排查：分页缺消息 → 游标语义为"严格更早 ts"，同毫秒多条消息会被游标跳过
（极端边界）；正常间隔发送不受影响。

### 2.10 回复消息（引用回复，IM-T46A/T46B）

| 步骤 | 操作 | 预期 |
|---|---|---|
| 1 | A 对 B 的某条消息发起回复（气泡悬停/右键菜单） | 输入区出现引用预览（被引用消息摘要），可取消退出 |
| 2 | A 输入文本发送 | A 侧新气泡带引用块，消息携带 replyTo（指向被引用消息 id） |
| 3 | B 侧查看入站消息 | 气泡渲染引用块；点击引用块滚动定位到被引用消息并高亮 |

失败排查：
- B 侧引用块缺失 → 查 B 的 messages/<peer>.jsonl 对应行是否含 replyTo 字段；
  旧消息/旧信封无此字段＝无引用，属兼容语义而非缺陷。
- 点击引用块无反应 → 被引用消息不在本地历史时显示占位文案，属预期降级
  （契约不校验被引用消息存在性，离线引用允许）。
- 发送时 replyTo 被拒 → 提供时须非空字符串，空白引用发送即 Err。

## 3. 标注（上限与依赖）

- **媒体上限**：单条消息（含附件原始字节）≤ 64 MiB（MAX_MESSAGE_SIZE）；
  超限发送前 Err（MediaTooLarge）、入站断流，禁止静默。
- **MIME 白名单**：image→png/jpeg/gif/webp；audio→mpeg/wav/ogg/m4a/mp4；
  video→mp4/webm/mov/quicktime；其余归 file（不匹配即 Err/断流）。
- **回复引用**：replyTo 为可选加法字段（被引用消息的本端消息 id），发送不校验
  被引用消息存在性（离线引用允许）；收端原样落盘，旧记录缺字段＝无引用，
  重启读回兼容（wire-protocol §8.1）。
- **asset URL 预览依赖 assetProtocol scope**：chat_media_file 返回 asset URL，
  内联预览（image/audio/video）依赖 Tauri assetProtocol 的 chat/media scope
  （src-tauri 已配置）；改动 tauri.conf.json 后必须回归本清单 2.3-2.6。

## 4. 观测点

- GUI 日志：`<app_log_dir>/p2p-console.log`（p2p-log 滚动文件，RUST_LOG 可调级别）；
  关键行：连接事件（connected/disconnected）、/im/chat/1 入站告警、
  outbox flush 结果（delivered/failed）。
- p2p-cli 节点：stdout 的 peer_id/listen_addrs；stderr 的 tracing 日志。
- 数据落盘（验收留证）：`<data_dir>/chat/friends.json`、
  `messages/<peer>.jsonl`（每条一行 ChatEnvelope，含 status）、
  `outbox/<peer>.jsonl`（离线未送达时非空）、`media/<peer>/`（附件文件）。

## 5. 回滚与急停

- 纯本机数据演练：异常时关 GUI/停 p2p-cli 即止，数据在本地 dataDir，无远端写面。
- 演练中任何一步异常：先留日志/落盘证据，再按「失败排查」重跑该步，禁止静默跳过；
  疑似实现缺陷 → 报告协调者，勿改生产代码。

## 6. 验收记录表模板

| 项 | 内容 |
|---|---|
| 日期 / 基线 | yyyy-mm-dd / main @ f732bc4 |
| 节点形态 | 双 Tauri（dataDir 隔离） / Tauri + p2p-cli |
| A peerId / B peerId | base58 各记前缀 |
| 2.1 文本互通 | 通过 / 失败（附日志行） |
| 2.2 emoji | 通过 / 失败 |
| 2.3 图片上传预览 | 通过 / 失败 |
| 2.4 音频上传播放 | 通过 / 失败 |
| 2.5 视频上传播放 | 通过 / 失败 |
| 2.6 任意文件下载 | 通过 / 失败 |
| 2.7 状态流转 sent→delivered | 通过 / 失败 |
| 2.8 离线消息自动补发 | 通过 / 失败 |
| 2.9 历史分页回读 | 通过 / 失败 |
| 2.10 回复消息引用 | 通过 / 失败 |
| 校准项命中 | §2.0 条目 + 实际行为 |
| 遗留问题 | 进入协调裁决/下一轮 |
