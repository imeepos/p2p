# IM 聊天系统设计 v1（冻结基线，2026-09-04）

状态：IM 阶段协调会话冻结。底座（p2p-* 内核）只读，本方案全部落在业务层：新建
`crates/p2p-chat` + `apps/gui`/`apps/gui/src-tauri` 消费面。契约来源：本文 §3/§4/§5 +
[gui-contract.md](gui-contract.md) §12。实现以本文件为准，违反即回退。

## 1. 定位与边界

IM = 好友间 1:1 私聊，消息可离线投递（重连即 flush），发件/收件/历史/附件全部落本机盘。

- **做**：好友簿管理（添加/删除/列表）、文本+emoji 消息（emoji 即 unicode 文本）、
  图片/音频/视频/文件附件（本机文件 → 线路上传 → 对端落盘可打开）、消息历史分页、
  发送状态可见（pending → sent → delivered）、离线队列（outbox，peers 连接事件触发重发）。
- **不做（本轮）**：实时音视频通话、群聊、已读回执、消息编辑/撤回、服务端消息中转
  （走既有 P2P 直连/降级链）、好友加好友双向确认流程（社交化发现 P2，另行立项）。
- **硬边界**：单条消息（含附件原始字节）≤ `64 MiB`（与 chunked.rs MAX_MESSAGE_SIZE 一致）；
  超限 → 显式 Err（可读中文）并留日志，禁止静默。附件 MIME 白名单按 kind 校验，拒绝即 Err。
  好友目标必须是合法 base58 PeerId；目标永不等于本机 PeerId。

## 2. 复用面（只读底座原语）

| 底座原语 | 用途 |
|---|---|
| `Node::connect(peer)` / `events()` `PeerConnected` | 发送前确保连接；outbox 重发触发点 |
| `Node::new_stream(peer, protocol)` | 出站消息流（首帧协议 ID 由底座写入） |
| `Node::handle_protocol(handler)` | 入站 `/im/chat/1` 分发 |
| chunked transfer（`chunked.rs`） | 附件字节分片（≤64MiB 重组上限） |
| `Node::peer_registered` / `add_peer_address` | 好友可拨性与地址登记 |
| `p2p-log` | 失败路径告警日志（禁止静默） |

## 3. 线协议 `/im/chat/1`

协议 ID：`/im/chat/1`（在 wire-protocol.md §8 登记，实现与登记同步提交）。
一条出站流 = 一个消息事务（send-once 语义）：开流 → 写信封帧 → 若有附件再写媒体块帧
→ 读 ack 帧 → 关流。入站流 = 读信封 → 收媒体 → 回 ack → 交给消息存储。

帧类型（每帧载荷首字节 = 类型头，与 chunked FRAME_* 同一风格，复用 §2 帧封装）：

| 类型头 | 值 | 载荷 |
|---|---|---|
| ENVELOPE | 0x01 | JSON 信封（§5 ChatEnvelope 的字节序列化） |
| MEDIA_BEGIN | 0x02 | 媒体头 JSON：`{len, name, mime, kind}`（单帧） |
| MEDIA_CHUNK | 0x03 | 原始附件分片（≤ 1 MiB/帧，chunked 规则） |
| ACK | 0x04 | JSON：`{id, ok, reason?}` —— 对端确认已收完整消息 |

- 时序：ENVELOPE →（可选 MEDIA_BEGIN → MEDIA_CHUNK×n）→ 对端 ACK。任意一帧校验失败
  （超上限/类型序非法/信封缺字段）→ 断流并留告警日志，发端收到失败即消息落 failed 态。
- 信封必须含 `sender` 字段，收端校验其与本流对端 PeerId 一致（防伪装，底座已保证流安全，
  此校验为纵深防御，不一致即断流告警）。
- 消息 id：发端生成 UUID（攻击面最小化，拒绝接受对端 dict 的 id）。
- wire-protocol.md 登记内容：协议 ID、四帧类型表、信封 JSON 字段、时序、64MiB 上限。

## 4. 本地存储布局（dataDir/chat/）

根目录在节点数据目录下 `chat/` 子目录（GUI 侧取 `app_data_dir()`；CLI 侧取 `--data-dir`）。

```
chat/
├── friends.json          # 好友簿：[{peerId, nickname, addrs[], note?}]，原子写（tmp+rename）
├── outbox/<peerId>.jsonl # 离线队列（追加式，发送成功后删行＝重写同文件）
└── messages/<peerId>.jsonl  # 消息历史（追加式，仅收/发本地校验通过的消息）
```

- messages 与 outbox 均为 JSONL：一行一条 ChatEnvelope；追加写、损坏行跳过并 warn。
- 附件落盘：`chat/media/<peerId>/<messageId>_<sanitizedName>`，sanitize = 去路径分隔符/
  控制字符，仅保留 `[A-Za-z0-9._-]`，空则回退 `attachment`；落盘用 tmp+rename。
- 非测试代码禁 unwrap/expect（panic-hygiene 门禁覆盖新 crate）。

## 5. 消息模型（GUI 契约 v7 数据形状，见 gui-contract.md §12）

```ts
type ChatKind = "text" | "image" | "audio" | "video" | "file";

interface ChatEnvelope {
  id: string;                 // UUID（发端生成）
  peer: string;               // 会话对端 PeerId（base58）
  sender: "me" | "them";
  kind: ChatKind;
  tsMs: number;               // 发端本地时间
  text?: string;              // kind=text 时有；trim 后 ≤2000 字符，空串禁止发送（Err）
  media?: {
    name: string;             // 原始文件名（sanitize 后展示）
    mime: string;             // 小写，白名单按 kind（image/audio/video 前缀匹配，其余归 file）
    size: number;             // 原始字节数
    path?: string;            // 本端落盘绝对路径（仅返回给本端展示用，不跨网）
  };
  status: "pending" | "sent" | "delivered" | "failed";  // 本地状态字段，不跨网
}
```

- kind 校验与 MIME 白名单：image→`image/png|jpeg|gif|webp`；
  audio→`audio/mpeg|wav|ogg|m4a|mp4`（mp4 音频容器容忍）；video→`video/mp4|webm|mov|quicktime`；
  其余一律视作 file（application/*、text/*、二进制兜底）。mime 与 kind 不匹配 → Err，不猜不降级。
- 发送状态机：`pending`（已入 outbox/发送中）→ `sent`（本端已写流）→ `delivered`（收 ACK）；
  断流/超时/校验失败 → `failed`（保留原因，日志留痕）。重启后 outbox 中未 sent 消息重新入队。

## 6. 发送与离线投递语义

1. `chat_send`：校验（peer 合法/文本或媒体合法/大小 ≤64MiB）→ 生成信封 → 落 outbox（异常原子性：
   先落盘后发送，发送失败不丢消息）→ 若对端已连接：开流发送；未连接：`connect()` 一次，
   失败则保持 pending，等待 `PeerConnected` 事件触发 flush。
2. outbox flush：`PeerConnected(peer)` 时重发该 peer 全部 pending 消息，成功置 delivered。
   flush 失败保留 outbox 条目 + 告警日志，不静默。
3. 入站：收完整消息（信封+媒体落盘）→ 回 ACK → 追加 messages/<peer>.jsonl →
   发 `chat_message` 事件给 GUI。
4. 历史：`chat_history(peer, beforeId?, limit≤100)` 返回 messages/<peer>.jsonl 倒序分页
   （beforeId 游标语义：返回比该 id 更早的消息；无 beforeId = 最新一页）。

## 7. 里程碑拆单（并行互斥，任务书只含需求与机械验收、不含源码）

| 单 | 范围（目录） | 依赖 | 验收要点 |
|---|---|---|---|
| T29 | 新建 crates/p2p-chat/**（协议/模型/好友簿/存储/outbox/发送接收）+ tests/ | 无 | cargo test -p p2p-chat 全绿；clippy -D warnings；双节点回环 itest（文本+附件+离线 flush+ACK）；wire-protocol.md §8 登记同步 |
| T30 | apps/gui/src：ipc-types/ipc/mock 的 chat 段、route 壳、menu.def.ts、i18n 中英、空聊天页 | 无（对 §5 契约编程） | pnpm build + pnpm test 全绿；i18n-diff 零漂移；make check |
| T31 | apps/gui/src/views/chat/** + components/chat/** + stores/chat-store.ts（会话列表/消息气泡/输入条/表情/附件/媒体预览/文件打开，mock 驱动） | T30 合入后 | pnpm build + pnpm test 全绿；交互组件测试（发文本/选表情/选附件/状态渲染）；make check |
| T32 | apps/gui/src-tauri/**：装配 p2p-chat、chat_* 命令与 chat_message/chat_status 事件、dataDir 接线、契约 roundtrip 测试 | T29 合入后 | src-tauri cargo test 全绿；pnpm build + make check；双命令冒烟（friends add/list） |
| T33 | 新增 crates/p2p-itest/tests/chat_e2e.rs + docs/ops/im-chat-drill.md + wire-protocol.md 复核 | T29+T32 合入后 | cargo test -p p2p-itest --test chat_e2e 全绿（两节点全链：加好友→文本→附件→重启 flush→历史回读）；make check；演练清单文档在位 |

## 8. 非目标登记（防范围蔓延）

- 实时音视频通话、群聊、已读回执、消息编辑/撤回、服务端中转、好友互认流程、端到端
  额外加密（传输层已 TLS1.3/Noise）、附件断点续传、跨端同步（本阶段单设备）。
