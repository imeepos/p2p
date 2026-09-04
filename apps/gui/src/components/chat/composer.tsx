import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ImagePlus, Send, Smile, X } from "lucide-react";

import { toastError } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { fileToChatMedia, inferKind } from "@/lib/chat-media";
import type { ChatKind, ChatMediaInput, ChatMessageJson, ChatSendReport } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { cn } from "@/lib/utils";

import { replyKindKey, replySummaryOf } from "./reply-summary";
import { EmojiPicker } from "./emoji-picker";
import { notifyFailedSendReport } from "./send-notify";

const MAX_TEXT_CHARS = 2000;

// 引用预览（IM-T46B）：被引用消息的类型化摘要 + 取消按钮；取消/清空即不带 replyTo。
function ReplyPreview({
  target,
  onCancel,
}: {
  target: ChatMessageJson;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const summary = replySummaryOf(target);
  return (
    <div
      className="mb-2 flex items-center gap-2 rounded-md border bg-muted/50 px-2 py-1.5 text-xs"
      data-testid="chat-reply-preview"
    >
      <span className="shrink-0 font-medium">{t("chat.reply.previewLabel")}</span>
      <span className="min-w-0 flex-1 truncate text-muted-foreground">
        {t(replyKindKey(target.kind))}
        {summary ? <span className="ml-1">{summary}</span> : null}
      </span>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="size-5 shrink-0"
        aria-label={t("chat.reply.cancel")}
        data-testid="chat-reply-cancel"
        onClick={onCancel}
      >
        <X aria-hidden className="size-3.5" />
      </Button>
    </div>
  );
}

// 群聊复用注入面（G3）：不传走 1:1 chat-store（默认路径，报告失败 toast 归本组件）；
// 传入则发送改道（群 store），报告级失败通知由传输侧自理。
export interface ComposerTransport {
  sendText(peer: string, text: string, replyTo?: string | null): Promise<unknown>;
  sendMedia(
    peer: string,
    kind: ChatKind,
    media: ChatMediaInput,
    replyTo?: string | null,
  ): Promise<unknown>;
}

// 输入条：多行文本 + 表情面板 + 附件；回车发送，shift+enter 换行；
// 空文本/超长禁用发送并提示；附件超限走 toastError（失败留信号）。
// 带引用时（replyTarget）文本与附件发送均透传 replyTo，成功后清预览。
export function Composer({
  peer,
  replyTarget,
  onReplyCancel,
  disabled = false,
  transport,
  testIds,
}: {
  peer: string;
  replyTarget: ChatMessageJson | null;
  onReplyCancel: () => void;
  /** 节点未运行等场景的外部禁用（IM-T51）：输入/发送/附件/表情全部不可用 */
  disabled?: boolean;
  transport?: ComposerTransport;
  /** 测试锚点（G3 群复用：group-input/group-send）；缺省保持 1:1 命名。 */
  testIds?: { input: string; send: string };
}) {
  const { t } = useTranslation();
  const ids = testIds ?? { input: "chat-input", send: "chat-send" };
  const sendText1v1 = useChatStore((s) => s.sendText);
  const sendMedia1v1 = useChatStore((s) => s.sendMedia);
  const tx: ComposerTransport = transport ?? {
    sendText: (to, text, replyTo) => sendText1v1(to, text, replyTo),
    sendMedia: (to, kind, media, replyTo) => sendMedia1v1(to, kind, media, replyTo),
  };
  const [text, setText] = useState("");
  const [emojiOpen, setEmojiOpen] = useState(false);
  const [sending, setSending] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const fileRef = useRef<HTMLInputElement | null>(null);

  const trimmed = text.trim();
  const tooLong = trimmed.length > MAX_TEXT_CHARS;
  const canSend = trimmed.length > 0 && !tooLong && !sending;

  const insertEmoji = (emoji: string) => {
    const el = textareaRef.current;
    const start = el?.selectionStart ?? text.length;
    const end = el?.selectionEnd ?? text.length;
    const next = text.slice(0, start) + emoji + text.slice(end);
    setText(next);
    requestAnimationFrame(() => {
      if (el) {
        el.focus();
        el.setSelectionRange(start + emoji.length, start + emoji.length);
      }
    });
  };

  const send = async () => {
    if (!canSend || disabled) return;
    setSending(true);
    try {
      const report = await tx.sendText(peer, trimmed, replyTarget?.id);
      setText("");
      onReplyCancel();
      if (!transport) notifyFailedSendReport(report as ChatSendReport);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      toastError(t("chat.sendFailed"), { description: reason });
    } finally {
      setSending(false);
    }
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void send();
    }
  };

  const pickFile = (file: File | undefined) => {
    if (!file) return;
    void (async () => {
      setSending(true);
      try {
        const media = await fileToChatMedia(file);
        const kind = inferKind(file.name, file.type);
        const report = await tx.sendMedia(peer, kind, media, replyTarget?.id);
        onReplyCancel();
        if (!transport) notifyFailedSendReport(report as ChatSendReport);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        console.error("[chat] 附件发送失败", error);
        toastError(t("chat.sendFailed"), { description: reason });
      } finally {
        setSending(false);
        if (fileRef.current) fileRef.current.value = "";
      }
    })();
  };

  return (
    <div className="shrink-0 border-t p-3">
      {replyTarget ? <ReplyPreview target={replyTarget} onCancel={onReplyCancel} /> : null}
      {emojiOpen ? (
        <div className="mb-2">
          <EmojiPicker onPick={insertEmoji} />
        </div>
      ) : null}
      <div className="flex items-end gap-2">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t("chat.emoji")}
          disabled={disabled}
          onClick={() => setEmojiOpen((open) => !open)}
        >
          <Smile className="size-4" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t("chat.attach")}
          disabled={sending || disabled}
          onClick={() => fileRef.current?.click()}
        >
          <ImagePlus className="size-4" />
        </Button>
        <input
          ref={fileRef}
          type="file"
          className="hidden"
          data-testid="chat-file-input"
          onChange={(event) => pickFile(event.target.files?.[0])}
        />
        <Textarea
          ref={textareaRef}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder={t("chat.inputPlaceholder")}
          aria-label={t("chat.inputPlaceholder")}
          data-testid={ids.input}
          disabled={disabled}
          className={cn("min-h-10 flex-1", tooLong && "border-destructive")}
        />
        <Button
          type="button"
          onClick={() => void send()}
          disabled={!canSend || disabled}
          data-testid={ids.send}
        >
          <Send className="size-4" />
          {t("chat.send")}
        </Button>
      </div>
      {tooLong ? (
        <p className="mt-1 text-xs text-destructive">{t("chat.textTooLong")}</p>
      ) : null}
    </div>
  );
}
