import type {
  ChatMediaFile,
  GroupDeliveryStatus,
  GroupJson,
  GroupMessageJson,
  GroupSendReport,
  IpcBackend,
} from "./ipc-types";
import { newGroupMessageId as uuid } from "./mock-group-state";
import {
  ACK_DELAY_MS,
  HISTORY_DEFAULT_LIMIT,
  HISTORY_MAX_LIMIT,
  REPLY_DELAY_MS,
  appendGroupMessage,
  bindMockGroupRuntime,
  delay,
  groupState,
  requireGroup,
  snapshotGroupMessage,
  type MockGroupChatDeps,
} from "./mock-group-state";
import {
  base64ByteSize,
  groupMediaPath,
  validateMessagePayload,
  validateReplyTo,
} from "./mock-chat-rules";
import { createMockGroupRosterOps, type MockGroupRosterBackend } from "./mock-group-roster";

// 契约群聊段（im-group-design §7）的 mock 侧组装：roster 操作面在
// mock-group-roster.ts，本文件承载发送 ack 事件流、历史分页与媒体路径。
// 持久化（groups.json/groups/<id>.jsonl/goutbox）由 src-tauri G4 落地，
// mock 只保内存态；与真实实现同签名（IpcBackend group 段）。

export type MockGroupChatBackend = MockGroupRosterBackend &
  Pick<IpcBackend, "groupSend" | "groupHistory" | "groupMediaFile">;

// 已连接成员逐个 ack（设计 §6.1）：每收一个 ACK 更新 acks 并发 chat_group_status；
// 全部目标确认后置 delivered。离线成员无 ack，条目保持 pending（goutbox 语义）。
async function collectAcks(
  deps: MockGroupChatDeps,
  group: GroupJson,
  message: GroupMessageJson,
  recipients: string[],
): Promise<string[]> {
  const connected = recipients.filter((member) => deps.isConnected(member));
  for (const member of connected) {
    await delay(ACK_DELAY_MS);
    message.acks = [...message.acks, member];
    const done = message.acks.length === recipients.length;
    if (done) message.status = "delivered";
    const status: GroupDeliveryStatus = done ? "delivered" : "pending";
    deps.emit({
      type: "chat_group_status",
      groupId: group.groupId,
      messageId: message.id,
      acks: [...message.acks],
      status,
    });
  }
  return connected;
}

// 已送达文本消息的成员回复：让 chat_group_message 入站事件在 mock 下可见可测。
function scheduleMockReply(
  deps: MockGroupChatDeps,
  group: GroupJson,
  message: GroupMessageJson,
  respondents: string[],
): void {
  if (message.kind !== "text" || message.text == null || respondents.length === 0) {
    return;
  }
  const text = message.text;
  const senderId = respondents[0]!;
  window.setTimeout(() => {
    const reply: GroupMessageJson = {
      id: uuid(),
      groupId: group.groupId,
      senderId,
      kind: "text",
      tsMs: Date.now(),
      text: `[mock 回复] 已收到：${text.slice(0, 40)}`,
      media: null,
      status: "delivered",
      acks: [],
    };
    appendGroupMessage(reply);
    deps.emit({
      type: "chat_group_message",
      groupId: group.groupId,
      message: snapshotGroupMessage(reply),
    });
  }, REPLY_DELAY_MS);
}

async function groupSend(
  deps: MockGroupChatDeps,
  groupId: string,
  kind: GroupMessageJson["kind"],
  text: string | undefined,
  media: Parameters<IpcBackend["groupSend"]>[3],
  replyTo: string | null | undefined,
): Promise<GroupSendReport> {
  const invalidReply = validateReplyTo(replyTo);
  if (invalidReply) throw new Error(invalidReply);
  const group = requireGroup(groupId);
  if (group.state !== "active") {
    throw new Error(`群当前不可用（${group.state}）：${groupId}`);
  }
  if (!group.members.includes(deps.selfPeerId())) {
    throw new Error(`你已不在该群：${groupId}`);
  }
  const invalid = validateMessagePayload(kind, text, media);
  if (invalid) throw new Error(invalid);
  const id = uuid();
  const message: GroupMessageJson = {
    id,
    groupId,
    senderId: deps.selfPeerId(),
    kind,
    tsMs: Date.now(),
    text: kind === "text" ? (text ?? "").trim() : null,
    media: media
      ? {
          name: media.name,
          mime: media.mime.toLowerCase(),
          size: base64ByteSize(media.dataBase64),
          path: groupMediaPath(groupId, id, media.name),
        }
      : null,
    status: "pending",
    acks: [],
    replyTo: replyTo ?? null,
  };
  appendGroupMessage(message);
  const recipients = group.members.filter((m) => m !== deps.selfPeerId());
  const connected = await collectAcks(deps, group, message, recipients);
  scheduleMockReply(deps, group, message, connected);
  return {
    message: snapshotGroupMessage(message),
    acked: connected.length,
    recipients: recipients.length,
    delivered: connected.length === recipients.length,
  };
}

// 时间 desc 分页：无 beforeId 取最新一页；beforeId 游标=严格更早（同 1:1）。
async function groupHistory(
  groupId: string,
  beforeId: string | null | undefined,
  limit: number | undefined,
): Promise<GroupMessageJson[]> {
  requireGroup(groupId);
  const requested = limit ?? HISTORY_DEFAULT_LIMIT;
  if (!Number.isInteger(requested) || requested <= 0) {
    throw new Error("limit 必须为正整数");
  }
  const size = Math.min(requested, HISTORY_MAX_LIMIT);
  const log = groupState.history.get(groupId) ?? [];
  let start = log.length;
  if (beforeId != null) {
    const cursor = log.findIndex((m) => m.id === beforeId);
    if (cursor < 0) throw new Error(`beforeId 对应消息不存在：${beforeId}`);
    start = cursor;
  }
  const page: GroupMessageJson[] = [];
  for (let i = start - 1; i >= 0 && page.length < size; i -= 1) {
    page.push(snapshotGroupMessage(log[i]!));
  }
  return page;
}

// 附件落盘路径占位（media/<groupId>/）；消息非 media 或不存在 → Err。
async function groupMediaFile(groupId: string, messageId: string): Promise<ChatMediaFile> {
  requireGroup(groupId);
  const log = groupState.history.get(groupId) ?? [];
  const message = log.find((m) => m.id === messageId);
  if (!message || !message.media) {
    throw new Error(`消息不存在或不是媒体消息：${messageId}`);
  }
  return {
    path: message.media.path ?? groupMediaPath(groupId, message.id, message.media.name),
    mime: message.media.mime,
    name: message.media.name,
  };
}

export function createMockGroupChatBackend(
  deps: MockGroupChatDeps,
): MockGroupChatBackend {
  // 场景注入运行时：mock-group-inject 与测试经此驱动入站群消息与外部群播种。
  bindMockGroupRuntime({
    emit: deps.emit,
    newId: uuid,
    appendMessage: (message) => {
      appendGroupMessage(message);
      return snapshotGroupMessage(message);
    },
    seedGroup: (group) => {
      groupState.groups.set(group.groupId, group);
      return { ...group, members: [...group.members] };
    },
  });
  return {
    ...createMockGroupRosterOps(deps),
    groupSend: (groupId, kind, text, media, replyTo) =>
      groupSend(deps, groupId, kind, text, media, replyTo),
    groupHistory,
    groupMediaFile,
  };
}
