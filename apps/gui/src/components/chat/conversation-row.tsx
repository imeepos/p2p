import { Bot, CircleAlert, Clock3, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { formatTime } from "@/lib/format";
import { formatUnreadCount } from "@/lib/conversation-entry";
import type { ConversationEntry } from "@/lib/conversation-entry";
import type { Locale } from "@/i18n";
import { cn } from "@/lib/utils";

// 统一会话行（§2.2）：渲染只消费 ConversationEntry，无来源分支。
// 未读 = 灰色圆点数字（≥100 显 99+，§2.3）；failed/error 显红色感叹角标，
// pending 显时钟图标；其余发送状态不显示（§2.2 条目发送状态呈现）。

const DOT_CLASS: Record<string, string> = {
  green: "bg-success",
  yellow: "bg-warning",
  red: "bg-destructive",
};

function KindMark({ entry }: { entry: ConversationEntry }) {
  const dot = entry.kindMark.dot;
  return (
    <span className="relative shrink-0">
      <span
        aria-hidden
        className={cn(
          "bg-muted text-muted-foreground flex size-9 items-center justify-center rounded-full text-sm font-medium",
          entry.kindMark.botIcon && "text-primary",
        )}
      >
        {entry.kindMark.botIcon ? (
          <Bot className="size-4" aria-hidden />
        ) : (
          entry.kindMark.initial
        )}
      </span>
      {entry.kindMark.groupBadge ? (
        <UsersRound
          aria-hidden
          className="text-muted-foreground absolute -bottom-0.5 -right-0.5 size-3.5 rounded-full bg-background"
        />
      ) : null}
      {dot ? (
        <span
          aria-hidden
          data-testid={`conversation-dot-${entry.id}`}
          className={cn(
            "absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-background",
            DOT_CLASS[dot],
          )}
        />
      ) : null}
    </span>
  );
}

export interface ConversationRowProps {
  entry: ConversationEntry;
  active: boolean;
  onSelect: (entry: ConversationEntry) => void;
}

export function ConversationRow({ entry, active, onSelect }: ConversationRowProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const showTime = entry.lastTsMs > 0;
  const sendStateIcon =
    entry.sendState === "failed" || entry.sendState === "error" ? (
      <CircleAlert
        aria-hidden
        data-testid={`conversation-sendstate-${entry.id}`}
        className="size-3.5 shrink-0 text-destructive"
      />
    ) : entry.sendState === "pending" ? (
      <Clock3
        aria-hidden
        data-testid={`conversation-sendstate-${entry.id}`}
        className="text-muted-foreground size-3.5 shrink-0"
      />
    ) : null;
  return (
    <li>
      <button
        type="button"
        onClick={() => onSelect(entry)}
        aria-current={active || undefined}
        data-testid={`conversation-row-${entry.kind}-${entry.id}`}
        className={cn(
          "hover:bg-accent flex w-full items-start gap-2 px-3 py-2 text-left text-sm",
          active && "bg-accent",
        )}
      >
        <KindMark entry={entry} />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex items-center justify-between gap-2">
            <span className="truncate font-medium">{entry.title}</span>
            {showTime ? (
              <time className="text-muted-foreground shrink-0 text-xs">
                {formatTime(entry.lastTsMs, locale)}
              </time>
            ) : null}
          </span>
          {entry.subtitle ? (
            <span className="text-muted-foreground truncate text-xs">{entry.subtitle}</span>
          ) : null}
          <span className="text-muted-foreground flex items-center gap-1 text-xs">
            {entry.statusBadge ? (
              <Badge variant={entry.statusBadge.tone} className="px-1 py-0 text-[10px]">
                {entry.statusBadge.label}
              </Badge>
            ) : null}
            <span className="min-w-0 flex-1 truncate">{entry.lastPreview ?? ""}</span>
            {sendStateIcon}
            {entry.unread > 0 ? (
              <span
                data-testid={`conversation-unread-${entry.id}`}
                aria-label={t("chat.unread.aria", { count: entry.unread })}
                className="bg-neutral-500 ml-auto inline-flex min-w-4 shrink-0 items-center justify-center rounded-full px-1 text-[10px] leading-4 font-medium text-white"
              >
                {formatUnreadCount(entry.unread)}
              </span>
            ) : null}
          </span>
        </span>
      </button>
    </li>
  );
}
