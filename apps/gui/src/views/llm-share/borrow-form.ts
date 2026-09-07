import type { I18nKey } from "@/i18n/types";

import { isValidFriendPeerId } from "@/views/contacts/chat-friend-rules";

import type { LlmBorrowReq } from "./types";

// borrow 快捷表单（§16.2-6 真实成本动作）：model/messages/maxTokens/targetPeer
// 全部必填，无缺省路径；reqId 客户端生成 UUID，重试复用（§16.2-3）。
export interface BorrowFormValues {
  targetPeer: string;
  model: string;
  messages: string;
  maxTokensText: string;
}

// maxTokens 给厂值缺省：可改但不必填想，借用路径少一步输入
export const EMPTY_BORROW_FORM: BorrowFormValues = {
  targetPeer: "",
  model: "",
  messages: "",
  maxTokensText: "1024",
};

export type BorrowField = "targetPeer" | "model" | "messages" | "maxTokens";
export type BorrowErrors = Partial<Record<BorrowField, I18nKey>>;

const KEY = "llmShare.borrow";

export interface BorrowValidation {
  errors: BorrowErrors;
  req: Omit<LlmBorrowReq, "reqId"> | null;
}

export function validateBorrowForm(values: BorrowFormValues): BorrowValidation {
  const errors: BorrowErrors = {};
  const targetPeer = values.targetPeer.trim();
  const model = values.model.trim();
  const messages = values.messages.trim();
  if (!targetPeer) errors.targetPeer = `${KEY}.errTargetPeerRequired` as I18nKey;
  // R2-05：与添加好友表单同口径——PeerId = base58 解码恰 32 字节
  else if (!isValidFriendPeerId(targetPeer)) {
    errors.targetPeer = `${KEY}.errTargetPeerFormat` as I18nKey;
  }
  if (!model) errors.model = `${KEY}.errModelRequired` as I18nKey;
  if (!messages) errors.messages = `${KEY}.errMessagesRequired` as I18nKey;
  const maxTokens = Number(values.maxTokensText.trim());
  if (!values.maxTokensText.trim() || !Number.isInteger(maxTokens) || maxTokens <= 0) {
    errors.maxTokens = `${KEY}.errMaxTokensRequired` as I18nKey;
  }
  const req =
    Object.keys(errors).length > 0
      ? null
      : { targetPeer, model, messages, maxTokens };
  return { errors, req };
}

export function newReqId(): string {
  const c = globalThis.crypto;
  if (c && typeof c.randomUUID === "function") return c.randomUUID();
  // jsdom 之外的极老环境兜底：仍保证同一次请求意图内值稳定
  return `req-${Date.now()}-${Math.floor(Math.random() * 1e9)}`;
}
