import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Bot, Loader2 } from "lucide-react";

import { MessageList } from "@/components/chat/message-list";
import { Composer } from "@/components/chat/composer";
import { EmptyState } from "@/views/shared/empty-state";
import { Badge } from "@/components/ui/badge";
import { ipc } from "@/lib/ipc";
import { useA2aStore } from "@/a2a/a2a-store";
import type { A2aTaskState, A2aMessage, A2aPart } from "@/a2a/task-types";
import type { ChatMessageJson } from "@/lib/ipc-types";
import type { BubbleAvatar } from "@/components/chat/message-bubble";
import type { I18nKey } from "@/i18n/types";

// A2A 会话记录区（docs/design/a2a-over-p2p-design.md §8.3）：
// 记录区复用 message-list/message-bubble（Q5 拍板=气泡栈，与 ACP transcript 栈分家）。
// task 状态徽章：StatusBadge（working=思考中/completed=完成/failed=失败/cancelled=已取消/rejected=已拒绝）。

const TASK_STATE_BADGE: Record<A2aTaskState, { variant: "default" | "secondary" | "destructive" | "outline"; labelKey: string }> = {
  submitted: { variant: "secondary", labelKey: "a2a.task.submitted" },
  working: { variant: "secondary", labelKey: "a2a.task.working" },
  completed: { variant: "outline", labelKey: "a2a.task.completed" },
  failed: { variant: "destructive", labelKey: "a2a.task.failed" },
  cancelled: { variant: "secondary", labelKey: "a2a.task.cancelled" },
  rejected: { variant: "destructive", labelKey: "a2a.task.rejected" },
};

interface A2aConversationProps {
  agentKey: string;
  hostPeer: string;
  agentId: string;
  agentName: string;
}

export function A2aConversation({ agentKey, hostPeer, agentId, agentName }: A2aConversationProps) {
  const { t } = useTranslation();
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);

  const a2aStore = useA2aStore();
  const task = taskId ? a2aStore.getTask(taskId) : undefined;

  const messages = task?.messages ?? [];
  const taskState = task?.state ?? null;

  // 任务通道凭据注入（gui-contract §17.1 同源 console WS）：未 connected 显式
  // 留痕（warn），首次发送以「任务通道未配置」原文上浮，禁静默。
  useEffect(() => {
    let dead = false;
    void ipc.acpConsoleStatus()
      .then((status) => {
        if (dead) return;
        if (status.phase === "connected" && status.wsUrl && status.token) {
          useA2aStore.getState().configureChannel(status.wsUrl, status.token);
        } else {
          console.warn("[a2a] console 未 connected：任务通道未配置（发送将失败）", status);
        }
      })
      .catch((error) => console.warn("[a2a] 任务通道凭据获取失败", error));
    return () => {
      dead = true;
    };
  }, []);

  const convertA2aMessageToChat = useCallback((msg: A2aMessage): ChatMessageJson => {
    const textPart = msg.parts.find((p): p is A2aPart & { type: "text" } => p.type === "text");
    return {
      id: msg.messageId || Math.random().toString(36).slice(2),
      peer: agentKey,
      sender: msg.role === "user" ? "me" : "them",
      kind: "text",
      text: textPart?.text || "",
      tsMs: msg.receivedAtMs ?? 0,
      status: "delivered",
    };
  }, [agentKey]);

  const chatMessages: ChatMessageJson[] = messages.map(convertA2aMessageToChat);

  const handleSend = useCallback(async (text: string) => {
    if (!text.trim() || isSending) return;
    setIsSending(true);
    try {
      const userMessage: A2aMessage = {
        role: "user",
        parts: [{ type: "text", text: text.trim() }],
      };
      if (!taskId) {
        const newTaskId = await a2aStore.createTask(hostPeer, agentId, userMessage);
        setTaskId(newTaskId);
      } else {
        await a2aStore.sendTaskMessage(taskId, userMessage);
      }
    } catch (error) {
      // 原文上抛：composer catch 统一 toast（chat.sendFailed + reason），禁静默
      console.error("[a2a] 发送失败:", error);
      throw error;
    } finally {
      setIsSending(false);
    }
  }, [taskId, hostPeer, agentId, isSending, a2aStore]);

  const avatar: BubbleAvatar = {
    label: agentName.slice(0, 1),
    seed: agentKey,
  };

  const badge = taskState ? TASK_STATE_BADGE[taskState] : null;

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-4 py-2 border-b">
        <Bot className="size-5" />
        <span className="font-medium">{agentName}</span>
        {badge && (
          <Badge variant={badge.variant}>
            {t(badge.labelKey as I18nKey)}
          </Badge>
        )}
        {taskState === "working" && (
          <Loader2 className="size-4 animate-spin" />
        )}
      </div>

      <div className="flex-1 overflow-hidden">
        {messages.length === 0 ? (
          <EmptyState
            icon={Bot}
            title={t("a2a.empty.title" as I18nKey)}
            description={t("a2a.empty.description" as I18nKey)}
          />
        ) : (
          <MessageList
            peer={agentKey}
            messages={chatMessages}
            loadingOlder={false}
            hasMore={false}
            onLoadOlder={() => {}}
            onCancelPending={() => {}}
            selfAvatar={{ label: t("chat.conversations.self" as I18nKey), seed: "self" }}
            peerAvatar={avatar}
          />
        )}
      </div>

      <Composer
        peer={agentKey}
        replyTarget={null}
        onReplyCancel={() => {}}
        disabled={isSending || taskState === "completed" || taskState === "failed"}
        transport={{
          sendText: async (_peer: string, text: string) => {
            await handleSend(text);
          },
          sendMedia: async () => {
            throw new Error("A2A v1 不支持媒体发送");
          },
        }}
      />
    </div>
  );
}