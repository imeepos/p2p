import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ImagePlus, Smile, X } from "lucide-react";

import { toastError } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useImeCompositionGuard } from "@/hooks/use-ime-composition";
import { fileToChatMedia, inferKind, resolveMime } from "@/lib/chat-media";
import { guardMediaFile, MEDIA_GUARD_I18N_KEY } from "@/lib/chat-limits";
import type { ChatKind, ChatMediaInput, ChatMessageJson, ChatSendReport } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { cn } from "@/lib/utils";

import { replyKindKey, replySummaryOf } from "./reply-summary";
import { EmojiPicker } from "./emoji-picker";
import { notifyFailedSendReport } from "./send-notify";

const MAX_TEXT_CHARS = 2000;
// F28：接近上限即显字数计数（UX 审计 20260907 F28：阈值取上限前 100 字）
const CHAR_COUNT_THRESHOLD = MAX_TEXT_CHARS - 100;

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
// IME 组合中（组合事件进行中或组合键码 229）Enter 视为确认候选词：
// 不发送、不拦截默认行为，确认后的按键才走发送；
// 空文本/超长禁用发送（§2.5 三律(1) 前置校验）；附件在读取前先走
// guardMediaFile 本地拦截（mime 白名单 + ≤64MiB + 空载荷），稳定错误码经
// i18n 渲染，不发无效请求；原始错误串只进提示详情。表单三律 (2)(3) 在
// composer 显式不适用：无候选集下拉场景，也不提供历史值下拉。
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
  const { compositionHandlers, shouldBlockEnter } = useImeCompositionGuard();

  const trimmed = text.trim();
  const tooLong = trimmed.length > MAX_TEXT_CHARS;
  const canSend = trimmed.length > 0 && !tooLong && !sending;
  // 计数按输入框原文计；超限判定沿用发送口径（trim 后），超限只提示不清空
  const charCount = text.length;
  const showCharCount = charCount >= CHAR_COUNT_THRESHOLD;

  const insertEmoji = (emoji: string) => {
    const el = textareaRef.current;
    const start = el?.selectionStart ?? text.length;
    const end = el?.selectionEnd ?? text.length;
    const next = text.slice(0, start) + emoji + text.slice(end);
    setText(next);
    // 选择成功即收起面板，焦点经下方 rAF 回落输入框
    setEmojiOpen(false);
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
      // 报告级失败统一上浮（1:1 与群同链路）：mark_failed 不抛错，失败禁止零解释
      notifyFailedSendReport(report as ChatSendReport);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      toastError(t("chat.sendFailed"), { description: reason });
    } finally {
      setSending(false);
    }
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // React 合成事件不透出 isComposing，判定须落在原生事件上
    if (shouldBlockEnter(event.nativeEvent)) return;
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void send();
    }
  };

  const pickFile = (file: File | undefined) => {
    if (!file) return;
    // §2.5 前置拦截在读取 base64 之前：超限/空/白名单外文件不进发送管线
    const kind = inferKind(file.name, file.type);
    const guardCode = guardMediaFile(kind, resolveMime(file.name, file.type), file.size);
    if (guardCode) {
      const reason = `guard=${guardCode} name=${file.name} size=${file.size}`;
      console.warn("[chat] 附件前置校验拦截", reason);
      toastError(t(MEDIA_GUARD_I18N_KEY[guardCode]), {
        description: reason,
      });
      if (fileRef.current) fileRef.current.value = "";
      return;
    }
    void (async () => {
      setSending(true);
      try {
        const media = await fileToChatMedia(file);
        const report = await tx.sendMedia(peer, kind, media, replyTarget?.id);
        onReplyCancel();
        notifyFailedSendReport(report as ChatSendReport);
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

  // WX1 微信风格输入条：图标工具行在上、无边框输入区居中、发送按钮沉底右。
  // 空内容/超长/发送中禁用（灰态）；可发送时按钮转微信绿。
  const sendActive = canSend && !disabled;
  return (
    <div
      className="shrink-0 border-t border-border/60 bg-background px-3 pt-1 pb-2.5"
      onKeyDown={(event) => {
        if (emojiOpen && event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          setEmojiOpen(false);
        }
      }}
    >
      {replyTarget ? <ReplyPreview target={replyTarget} onCancel={onReplyCancel} /> : null}
      {emojiOpen ? (
        <div className="mb-2" id="chat-emoji-picker">
          <EmojiPicker onPick={insertEmoji} />
        </div>
      ) : null}
      <div className="flex items-center gap-0.5">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 rounded-md"
          aria-label={t("chat.emoji")}
          aria-expanded={emojiOpen}
          aria-controls="chat-emoji-picker"
          disabled={disabled}
          onClick={() => setEmojiOpen((open) => !open)}
        >
          <Smile className="size-4.5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 rounded-md"
          aria-label={t("chat.attach")}
          disabled={sending || disabled}
          onClick={() => fileRef.current?.click()}
        >
          <ImagePlus className="size-4.5" />
        </Button>
        <input
          ref={fileRef}
          type="file"
          className="hidden"
          data-testid="chat-file-input"
          onChange={(event) => pickFile(event.target.files?.[0])}
        />
      </div>
      <Textarea
        ref={textareaRef}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={onKeyDown}
        {...compositionHandlers}
        placeholder={t("chat.inputPlaceholder")}
        aria-label={t("chat.inputPlaceholder")}
        data-testid={ids.input}
        disabled={disabled}
        className={cn(
          "min-h-16 resize-none border-0 bg-transparent px-1 py-1.5 shadow-none focus-visible:ring-0 dark:bg-transparent",
          tooLong && "text-destructive",
        )}
      />
      <div className="flex min-h-8 items-center gap-2">
        {showCharCount || tooLong ? (
          <>
            {tooLong ? (
              <p className="text-destructive text-xs" role="alert" data-testid="chat-text-too-long">
                {t("chat.textTooLong")}
              </p>
            ) : null}
            <span
              data-testid="chat-char-count"
              aria-live="polite"
              className={cn(
                "text-muted-foreground ml-auto shrink-0 text-xs tabular-nums",
                tooLong && "text-destructive",
              )}
            >
              {t("chat.charCount", { count: charCount, max: MAX_TEXT_CHARS })}
            </span>
          </>
        ) : null}
        <Button
          type="button"
          size="sm"
          className={cn(
            "ml-auto min-w-18",
            sendActive
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-secondary text-muted-foreground border-border/60 border",
          )}
          onClick={() => void send()}
          disabled={!canSend || disabled}
          data-testid={ids.send}
        >
          {sending ? t("settings.saveBar.saving") : t("chat.send")}
        </Button>
      </div>
    </div>
  );
}