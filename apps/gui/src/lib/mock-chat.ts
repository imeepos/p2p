import type {
  ChatFriendJson,
  FriendInviteJson,
  ChatMediaFile,
  ChatMessageJson,
  IpcBackend,
  NodeEventJson,
} from "./ipc-types";
import {
  MAX_NICKNAME_CHARS,
  base64ByteSize,
  isValidPeerId,
  isValidTransportAddr,
  mediaPath,
  validateGroupName,
  validateReplyTo,
  validateSend,
} from "./mock-chat-rules";

// 契约 v7 §12 的 mock 侧实现（T30）：好友簿校验、发送状态事件、历史分页与
// 媒体路径占位。持久化（friends.json/outbox/messages/media）由 src-tauri T32
// 落地，mock 只保内存态；与真实实现同签名（IpcBackend chat 段）。
// 校验规则在 mock-chat-rules.ts（与 mock/src-tauri/p2p-chat 同口径）。

const HISTORY_DEFAULT_LIMIT = 50;
const HISTORY_MAX_LIMIT = 100;
const SENT_DELAY_MS = 120;
const DELIVERED_DELAY_MS = 180;
const REPLY_DELAY_MS = 400;

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
  | "chatFriendInvite"
  | "chatInvitesList"
  | "chatInviteAccept"
  | "chatInviteReject"
  | "chatInviteCancel"
  | "chatFriendRemove"
  | "chatFriendUpdate"
  | "chatHistory"
  | "chatSend"
  | "chatMediaFile"
> & {
  // 测试引导直加入口（不属 IpcBackend 契约面）
  chatFriendAdd(
    peerId: string,
    nickname: string,
    addrs: string[],
  ): Promise<ChatFriendJson>;
};

interface MockChatState {
  friends: Map<string, ChatFriendJson>;
  invites: FriendInviteJson[]; // 邀请簿镜像（out+in）
  history: Map<string, ChatMessageJson[]>; // 每 peer 时间升序追加
}

const state: MockChatState = { friends: new Map(), invites: [], history: new Map() };

// 群成员资格的数据源：群聊后端经此判定成员是否在好友簿（im-group-design §1）。
export function isMockFriend(peerId: string): boolean {
  return state.friends.has(peerId);
}

// 场景注入运行时（IM-T50）：后端创建即登记，mock-chat-inject 经此驱动
// 历史与事件通道；不改变 IpcBackend 契约面，不入 prod bundle。
export interface MockChatRuntime {
  emit(event: NodeEventJson): void;
  newId(): string;
  appendMessage(message: ChatMessageJson): ChatMessageJson;
  findMessage(peer: string, messageId: string): ChatMessageJson | undefined;
}

let activeRuntime: MockChatRuntime | null = null;

// 未初始化时显式抛错（可观测信号），不静默。
export function activeMockChatRuntime(): MockChatRuntime {
  if (!activeRuntime) {
    throw new Error("mock chat 后端未初始化：先调用 createMockChatBackend");
  }
  return activeRuntime;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function uuid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  // 旧运行时兜底：非加密随机仅影响 mock 演示，不影响协议语义。
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
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
  activeRuntime = {
    emit: deps.emit,
    newId: uuid,
    appendMessage: (message) => {
      appendMessage(message);
      return snapshotMessage(message);
    },
    findMessage: (peer, messageId) =>
      state.history.get(peer)?.find((m) => m.id === messageId),
  };
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

    // 测试引导入口（直建好友簿，等价 crate friend_add_direct）；用户路径走 chatFriendInvite。
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

    // 邀请制加好友（契约 v9 §12.4）：mock 对端恒在线，INVITE 即时送达。
    async chatFriendInvite(peerId, nickname, addrs) {
      if (!isValidPeerId(peerId)) {
        throw new Error(`peerId 非法（需 base58，43-45 字符）：${peerId}`);
      }
      if (peerId === deps.selfPeerId()) {
        throw new Error(`不能把自己加为好友：${peerId}`);
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
      const invite: FriendInviteJson = {
        peerId,
        nickname: name,
        addrs: [...addrs],
        note: null,
        direction: "out",
        tsMs: Date.now(),
        delivered: true,
      };
      state.invites = state.invites.filter(
        (i) => !(i.peerId === peerId && i.direction === "out"),
      );
      state.invites.push(invite);
      deps.addKnownPeer(peerId);
      return { invite: { ...invite, addrs: [...invite.addrs] }, delivered: true };
    },

    async chatInvitesList() {
      return state.invites.map((i) => ({ ...i, addrs: [...i.addrs] }));
    },

    async chatInviteAccept(peerId, nickname) {
      const invite = state.invites.find(
        (i) => i.peerId === peerId && i.direction === "in",
      );
      if (!invite) {
        throw new Error(`无待处理邀请：${peerId}`);
      }
      if (state.friends.has(peerId)) {
        throw new Error(`该节点已是好友：${peerId}`);
      }
      const name = nickname.trim() || invite.nickname;
      const friend: ChatFriendJson = {
        peerId,
        nickname: name,
        addrs: [...invite.addrs],
        note: null,
      };
      state.friends.set(peerId, friend);
      deps.addKnownPeer(peerId);
      state.invites = state.invites.filter((i) => i.peerId !== peerId);
      return { ...friend, addrs: [...friend.addrs] };
    },

    async chatInviteReject(peerId) {
      const before = state.invites.length;
      state.invites = state.invites.filter(
        (i) => !(i.peerId === peerId && i.direction === "in"),
      );
      if (state.invites.length === before) {
        throw new Error(`无待处理邀请：${peerId}`);
      }
    },

    async chatInviteCancel(peerId) {
      const before = state.invites.length;
      state.invites = state.invites.filter(
        (i) => !(i.peerId === peerId && i.direction === "out"),
      );
      return state.invites.length !== before;
    },

    // 幂等：不在簿返回 false；不删消息历史（契约 §12.1）。
    async chatFriendRemove(peerId) {
      return state.friends.delete(peerId);
    },

    // 资料补丁（IM-T43）：group/nickname/note 至少一项；空串 group = 移出分组；
    // 与 p2p-chat friend_update 同口径：peer 不在簿/越界组名/空补丁均拒绝。
    async chatFriendUpdate(peerId, patch) {
      const friend = state.friends.get(peerId);
      if (!friend) {
        throw new Error(`好友不在簿：${peerId}`);
      }
      if (patch.group == null && patch.nickname == null && patch.note == null) {
        throw new Error("更新内容为空：group/nickname/note 至少提供一项");
      }
      if (patch.nickname != null) {
        const name = patch.nickname.trim();
        if (name.length > MAX_NICKNAME_CHARS) {
          throw new Error(`nickname 超过 ${MAX_NICKNAME_CHARS} 字符上限`);
        }
        friend.nickname = name;
      }
      if (patch.group != null) {
        const invalid = validateGroupName(patch.group);
        if (invalid) throw new Error(invalid);
        const trimmed = patch.group.trim();
        // 未分组不落盘空串（契约裁决：None/空串统一为未分组）
        friend.group = trimmed.length > 0 ? trimmed : null;
      }
      if (patch.note != null) {
        const note = patch.note.trim();
        friend.note = note.length > 0 ? note : null;
      }
      return { ...friend, addrs: [...friend.addrs] };
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

    // replyTo 透传（IM-T46B）：先校验非空（镜像 crate 侧 InvalidReply），原样入库。
    async chatSend(peer, kind, text, media, replyTo) {
      const invalidReply = validateReplyTo(replyTo);
      if (invalidReply) throw new Error(invalidReply);
      const invalid = validateSend(peer, kind, text, media, state.friends.has(peer));
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
        replyTo: replyTo ?? null,
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
