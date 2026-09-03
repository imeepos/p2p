import type {
  ChatFriendJson,
  ChatKind,
  ChatMediaFile,
  ChatMediaInput,
  ChatMessageJson,
  IpcBackend,
  NodeEventJson,
} from "./ipc-types";
import { isValidPeerId, isValidTransportAddr } from "./dial-target";

// 契约 v7 §12 的 mock 侧实现（T30）：好友簿校验、发送状态事件、历史分页与
// 媒体路径占位。持久化（friends.json/outbox/messages/media）由 src-tauri T32
// 落地，mock 只保内存态；与真实实现同签名（IpcBackend chat 段）。

const MAX_TEXT_CHARS = 2000;
const MAX_MEDIA_BYTES = 64 * 1024 * 1024; // 与 chunked.rs MAX_MESSAGE_SIZE 一致
const HISTORY_DEFAULT_LIMIT = 50;
const HISTORY_MAX_LIMIT = 100;
const SENT_DELAY_MS = 120;
const DELIVERED_DELAY_MS = 180;
const REPLY_DELAY_MS = 400;
const MAX_NICKNAME_CHARS = 64;

// 设计 §5 MIME 白名单：kind 与 mime 不匹配一律 Err，不猜不降级。
const MIME_BY_KIND: Record<
  Exclude<ChatKind, "text" | "file">,
  ReadonlySet<string>
> = {
  image: new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]),
  audio: new Set([
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/m4a",
    "audio/mp4",
  ]),
  video: new Set(["video/mp4", "video/webm", "video/mov", "video/quicktime"]),
};

const BASE64_RE = /^[A-Za-z0-9+/]+={0,2}$/;

// mock 运行时接线：读 mock-ipc 的节点态（本机 PeerId/运行/连接），发 chat 事件。
export interface MockChatDeps {
  emit(event: NodeEventJson): void;
  selfPeerId(): string;
  isRunning(): boolean;
  isConnected(peer: string): boolean;
  addKnownPeer(peerId: string): void;
}

export type MockChatBackend = Pick<
  IpcBackend,
  | "chatFriendsList"
  | "chatFriendAdd"
  | "chatFriendRemove"
  | "chatHistory"
  | "chatSend"
  | "chatMediaFile"
>;

interface MockChatState {
  friends: Map<string, ChatFriendJson>;
  history: Map<string, ChatMessageJson[]>; // 每 peer 时间升序追加
}

const state: MockChatState = { friends: new Map(), history: new Map() };

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function uuid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  // 旧运行时兜底：非加密随机仅影响 mock 演示，不影响协议语义。
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

// 设计 §4：去路径分隔符/控制字符，仅保留 [A-Za-z0-9._-]，空则回退 attachment。
function sanitizeName(name: string): string {
  const kept = name.replace(/[^A-Za-z0-9._-]/g, "");
  return kept.length > 0 ? kept : "attachment";
}

function mediaPath(peer: string, messageId: string, name: string): string {
  return `<app-data>/chat/media/${peer}/${messageId}_${sanitizeName(name)}`;
}

function base64ByteSize(dataBase64: string): number {
  const padding = dataBase64.endsWith("==") ? 2 : dataBase64.endsWith("=") ? 1 : 0;
  return Math.floor((dataBase64.length / 4) * 3) - padding;
}

function expectedKind(mime: string): ChatKind {
  for (const [kind, set] of Object.entries(MIME_BY_KIND)) {
    if (set.has(mime)) return kind as ChatKind;
  }
  return "file";
}

function validateMedia(kind: ChatKind, media: ChatMediaInput): string | null {
  const mime = media.mime.toLowerCase();
  if (expectedKind(mime) !== kind) {
    return `媒体 mime 与 kind 不匹配：${media.mime} 不能作为 ${kind} 发送`;
  }
  if (!BASE64_RE.test(media.dataBase64)) return "附件 base64 载荷非法";
  const size = base64ByteSize(media.dataBase64);
  if (size > MAX_MEDIA_BYTES) {
    return `附件超过单条消息上限（${size} > ${MAX_MEDIA_BYTES} 字节）`;
  }
  return null;
}

function validateSend(
  peer: string,
  kind: ChatKind,
  text: string | undefined,
  media: ChatMediaInput | undefined,
): string | null {
  if (!state.friends.has(peer)) return `对方还不是好友：${peer}`;
  if (kind === "text") {
    const trimmed = (text ?? "").trim();
    if (trimmed.length === 0) return "文本消息不能为空";
    if (trimmed.length > MAX_TEXT_CHARS) {
      return `文本超过 ${MAX_TEXT_CHARS} 字符上限`;
    }
    return null;
  }
  if (!media) return `kind=${kind} 的消息必须携带 media`;
  return validateMedia(kind, media);
}

function appendMessage(message: ChatMessageJson): void {
  const log = state.history.get(message.peer) ?? [];
  log.push(message);
  state.history.set(message.peer, log);
}

function snapshotMessage(message: ChatMessageJson): ChatMessageJson {
  return { ...message, media: message.media ? { ...message.media } : null };
}

export function createMockChatBackend(deps: MockChatDeps): MockChatBackend {
  // 已送达文本消息的脚本化回复：让 chat_message 入站事件在 mock 下可见可测。
  function scheduleMockReply(peer: string, source: ChatMessageJson): void {
    if (source.kind !== "text" || !source.text) return;
    const text = source.text;
    window.setTimeout(() => {
      const reply: ChatMessageJson = {
        id: uuid(),
        peer,
        sender: "them",
        kind: "text",
        tsMs: Date.now(),
        text: `[mock 回复] 已收到：${text.slice(0, 40)}`,
        media: null,
        status: "delivered",
      };
      appendMessage(reply);
      deps.emit({ type: "chat_message", peer, message: snapshotMessage(reply) });
    }, REPLY_DELAY_MS);
  }

  return {
    async chatFriendsList() {
      return [...state.friends.values()].map((f) => ({
        ...f,
        addrs: [...f.addrs],
      }));
    },

    // 校验镜像契约 §12.1：peerId base58 且不等于本机、nickname trim ≤64、addr 逐条校验。
    async chatFriendAdd(peerId, nickname, addrs) {
      if (!isValidPeerId(peerId)) {
        throw new Error(`peerId 非法（需 base58，43-45 字符）：${peerId}`);
      }
      if (peerId === deps.selfPeerId()) {
        throw new Error("不能把自己加为好友");
      }
      const name = nickname.trim();
      if (name.length > MAX_NICKNAME_CHARS) {
        throw new Error(`nickname 超过 ${MAX_NICKNAME_CHARS} 字符上限`);
      }
      for (const addr of addrs) {
        if (!isValidTransportAddr(addr)) {
          throw new Error(
            `好友地址语法非法（应为 ip/u端口 或 ip/t端口）：${addr}`,
          );
        }
      }
      if (state.friends.has(peerId)) {
        throw new Error(`该节点已是好友：${peerId}`);
      }
      const friend: ChatFriendJson = {
        peerId,
        nickname: name,
        addrs: [...addrs],
        note: null,
      };
      state.friends.set(peerId, friend);
      deps.addKnownPeer(peerId); // addr 同时登记地址簿可拨（契约 §12.1）
      return { ...friend, addrs: [...friend.addrs] };
    },

    // 幂等：不在簿返回 false；不删消息历史（契约 §12.1）。
    async chatFriendRemove(peerId) {
      return state.friends.delete(peerId);
    },

    // 时间 desc 分页：无 beforeId 取最新一页；beforeId 游标=严格更早（设计 §6.4）。
    async chatHistory(peer, beforeId, limit) {
      const requested = limit ?? HISTORY_DEFAULT_LIMIT;
      if (!Number.isInteger(requested) || requested <= 0) {
        throw new Error("limit 必须为正整数");
      }
      const size = Math.min(requested, HISTORY_MAX_LIMIT);
      const log = state.history.get(peer) ?? [];
      let start = log.length;
      if (beforeId != null) {
        const cursor = log.findIndex((m) => m.id === beforeId);
        if (cursor < 0) {
          throw new Error(`beforeId 对应消息不存在：${beforeId}`);
        }
        start = cursor;
      }
      const page: ChatMessageJson[] = [];
      for (let i = start - 1; i >= 0 && page.length < size; i -= 1) {
        page.push(snapshotMessage(log[i]!));
      }
      return page;
    },

    async chatSend(peer, kind, text, media) {
      const invalid = validateSend(peer, kind, text, media);
      if (invalid) throw new Error(invalid);
      const id = uuid();
      const message: ChatMessageJson = {
        id,
        peer,
        sender: "me",
        kind,
        tsMs: Date.now(),
        text: kind === "text" ? (text ?? "").trim() : null,
        media: media
          ? {
              name: media.name,
              mime: media.mime.toLowerCase(),
              size: base64ByteSize(media.dataBase64),
              path: mediaPath(peer, id, media.name),
            }
          : null,
        status: "pending",
      };
      appendMessage(message);
      // 离线语义（设计 §6.1）：先落历史（outbox 化），未连接保持 pending；
      // mock 不模拟 PeerConnected 触发的 outbox flush，重启即弃（内存态）。
      if (!deps.isRunning() || !deps.isConnected(peer)) {
        return { message: snapshotMessage(message), delivered: false };
      }
      await delay(SENT_DELAY_MS);
      message.status = "sent";
      deps.emit({ type: "chat_status", peer, messageId: id, status: "sent" });
      await delay(DELIVERED_DELAY_MS);
      message.status = "delivered";
      deps.emit({
        type: "chat_status",
        peer,
        messageId: id,
        status: "delivered",
      });
      scheduleMockReply(peer, message);
      return { message: snapshotMessage(message), delivered: true };
    },

    // 媒体占位：返回设计 §4 布局下的落盘路径形状；真实文件由 T32 写盘。
    async chatMediaFile(peer, messageId) {
      const log = state.history.get(peer) ?? [];
      const message = log.find((m) => m.id === messageId);
      if (!message || !message.media) {
        throw new Error(`消息不存在或不是媒体消息：${messageId}`);
      }
      const file: ChatMediaFile = {
        path: message.media.path ?? mediaPath(peer, message.id, message.media.name),
        mime: message.media.mime,
        name: message.media.name,
      };
      return file;
    },
  };
}
