import type { ChatKind, ChatMediaInput, GroupJson, GroupMessageJson } from "./ipc-types";
import { activeMockGroupChatRuntime } from "./mock-group-state";
import { base64ByteSize, groupMediaPath } from "./chat-limits";

// mock 群场景注入：dev 演示与集成测试经 window.__MOCK_GROUP__ 驱动
// 入站群消息（chat_group_message 事件通道）与外部群播种（owner ≠ 本机）。
// 注入是测试后门，不做成员资格校验（roster 前置不适用）。

export interface MockGroupIncomingInput {
  senderId: string;
  kind: ChatKind;
  text?: string;
  media?: ChatMediaInput;
}

function validateIncoming(input: MockGroupIncomingInput): string | null {
  if (!input.senderId.trim()) return "注入群消息需要 senderId";
  if (input.kind === "text" && !(input.text ?? "").trim()) {
    return "注入 text 消息需要非空 text";
  }
  if (input.kind !== "text" && !input.media) {
    return `注入 ${input.kind} 消息需要 media`;
  }
  return null;
}

// 注入群成员消息：落历史并发 chat_group_message；返回快照供测试断言。
export function injectMockGroupIncoming(
  groupId: string,
  input: MockGroupIncomingInput,
): GroupMessageJson {
  const runtime = activeMockGroupChatRuntime();
  const invalid = validateIncoming(input);
  if (invalid) throw new Error(invalid);
  const id = runtime.newId();
  const message: GroupMessageJson = {
    id,
    groupId,
    senderId: input.senderId,
    kind: input.kind,
    tsMs: Date.now(),
    text: input.kind === "text" ? (input.text ?? "").trim() : null,
    media: input.media
      ? {
          name: input.media.name,
          mime: input.media.mime.toLowerCase(),
          size: base64ByteSize(input.media.dataBase64),
          path: groupMediaPath(groupId, id, input.media.name),
        }
      : null,
    status: "delivered",
    acks: [],
  };
  const snapshot = runtime.appendMessage(message);
  runtime.emit({ type: "chat_group_message", groupId, message: snapshot });
  return snapshot;
}

// 播种外部群（owner ≠ 本机场景，如非 owner 操作面测试）：原样入册，返回快照。
export function seedMockGroup(group: GroupJson): GroupJson {
  return activeMockGroupChatRuntime().seedGroup(group);
}
