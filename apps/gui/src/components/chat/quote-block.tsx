import { useTranslation } from "react-i18next";

import type { ChatKind } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";

import { replyKindKey } from "./reply-summary";

interface QuoteBlockProps {
  // kind：被引用消息类型；目标不在本地时缺省。
  // missing：被引用消息不在本地历史，原位占位文案不白屏。
  // tone：嵌套底色跟随所在气泡配色（me 深底 / them 浅底）。
  kind?: ChatKind;
  summary: string | null;
  missing: boolean;
  tone: "me" | "them";
  onOpen: () => void;
}

// 引用块：类型标识 + 摘要；点击跳转到被引用消息；缺失时仅显示占位文案。
export function QuoteBlock({ kind, summary, missing, tone, onOpen }: QuoteBlockProps) {
  const { t } = useTranslation();
  if (missing) {
    return (
      <div
        data-testid="chat-quote-missing"
        className={cn(
          "mb-1 rounded border-l-2 px-2 py-1 text-xs opacity-80",
          tone === "me"
            ? "border-primary-foreground/40 bg-primary-foreground/10"
            : "border-foreground/20 bg-background/60",
        )}
      >
        {t("chat.reply.quotedMissing")}
      </div>
    );
  }
  return (
    <button
      type="button"
      data-testid="chat-quote-block"
      onClick={onOpen}
      title={t("chat.reply.jump")}
      className={cn(
        "mb-1 block w-full rounded border-l-2 px-2 py-1 text-left text-xs hover:opacity-90",
        tone === "me"
          ? "border-primary-foreground/40 bg-primary-foreground/10"
          : "border-foreground/20 bg-background/60",
      )}
    >
      <span className="font-medium">{kind ? t(replyKindKey(kind)) : ""}</span>
      {summary ? <span className="ml-1 opacity-80">{summary}</span> : null}
    </button>
  );
}
