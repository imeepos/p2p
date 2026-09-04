import type {
  ChatKind,
  ChatMediaInput,
  ChatMessageJson,
  ChatMessageStatus,
} from "./ipc-types";
import { activeMockChatRuntime } from "./mock-chat";
import { base64ByteSize, mediaPath } from "./mock-chat-rules";

// mock 场景注入（IM-T50）：dev 演示与集成测试的全状态矩阵驱动——
// them 气泡注入与消息状态强制推进走与真实后端同构的
// chat_message/chat_status 事件通道；历史态与事件视图保持一致。
// 注入是测试后门，不做好友簿校验（validateSend 的好友前置不适用）。

export interface MockIncomingInput {
  kind: ChatKind;
  text?: string;
  media?: ChatMediaInput;
  status?: ChatMessageStatus;
}

function validateIncoming(input: MockIncomingInput): string | null {
  if (input.kind === "text" && !(input.text ?? "").trim()) {
    return "注入 text 消息需要非空 text";
  }
  if (input.kind !== "text" && !input.media) {
    return `注入 ${input.kind} 消息需要 media`;
  }
  return null;
}

// 注入对方（them）消息：落历史并发 chat_message；返回快照供测试断言。
export function injectMockIncoming(
  peer: string,
  input: MockIncomingInput,
): ChatMessageJson {
  const runtime = activeMockChatRuntime();
  const invalid = validateIncoming(input);
  if (invalid) throw new Error(invalid);
  const id = runtime.newId();
  const message: ChatMessageJson = {
    id,
    peer,
    sender: "them",
    kind: input.kind,
    tsMs: Date.now(),
    text: input.kind === "text" ? (input.text ?? "").trim() : null,
    media: input.media
      ? {
          name: input.media.name,
          mime: input.media.mime.toLowerCase(),
          size: base64ByteSize(input.media.dataBase64),
          path: mediaPath(peer, id, input.media.name),
        }
      : null,
    status: input.status ?? "delivered",
  };
  const snapshot = runtime.appendMessage(message);
  runtime.emit({ type: "chat_message", peer, message: snapshot });
  return snapshot;
}

// 强制推进消息状态（如 pending→failed）：改历史态并发 chat_status 事件；
// 目标不存在显式抛错，不静默。
export function forceMockMessageStatus(
  peer: string,
  messageId: string,
  status: ChatMessageStatus,
): ChatMessageJson {
  const runtime = activeMockChatRuntime();
  const message = runtime.findMessage(peer, messageId);
  if (!message) {
    throw new Error(`注入目标消息不存在：peer=${peer} id=${messageId}`);
  }
  message.status = status;
  const snapshot = {
    ...message,
    media: message.media ? { ...message.media } : null,
  };
  runtime.emit({ type: "chat_status", peer, messageId, status });
  return snapshot;
}
