import { useTranslation } from "react-i18next";

import { toastError } from "@/components/feedback/toast";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

import { notifyFailedSendReport } from "./send-notify";

// 失败文本重发（IM-T51）：以原文本与原 replyTo 重新走乐观发送；
// IPC 抛错与 mark_failed 报告两条失败路径都留可读信号，不静默。
export function useRetrySend(
  peer: string | null,
): (message: ChatMessageJson) => Promise<void> {
  const { t } = useTranslation();
  const sendText = useChatStore((s) => s.sendText);
  return async (message) => {
    if (!peer || message.kind !== "text" || !message.text) return;
    try {
      notifyFailedSendReport(
        await sendText(peer, message.text, message.replyTo ?? undefined),
      );
    } catch (error) {
      console.error("[chat] 重试发送失败", error);
      toastError(t("chat.sendFailed"), {
        description: error instanceof Error ? error.message : String(error),
        context: "chat.retry",
      });
    }
  };
}
