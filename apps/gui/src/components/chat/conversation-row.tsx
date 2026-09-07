import { Bot, CircleAlert, Clock3, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AvatarBox } from "@/components/chat/avatar-box";
import { Badge } from "@/components/ui/badge";
import { formatConversationTime } from "@/lib/format";
import { formatUnreadCount } from "@/lib/conversation-entry";
import type { ConversationEntry } from "@/lib/conversation-entry";
import type { Locale } from "@/i18n";
import { cn } from "@/lib/utils";

// WX1 微信风格会话行：方形头像 + 名称/时间行 + 预览行；选中态微信绿底白字。
// 未读红点角标（≥100 显 99+，§2.3）；failed/error 显红色感叹角标，pending 显
// 时钟图标；头像右上在线状态点保留（功能位）。

const DOT_CLASS: Record<string, string> = {
  green: "bg-success",
  yellow: "bg-warning",
  red: "bg-destructive",
};

function RowAvatar({ entry, active }: { entry: ConversationEntry; active: boolean }) {
  const dot = entry.kindMark.dot;
  return (
    <span className="relative shrink-0">
      <AvatarBox
        label={entry.title}
        seed={entry.id}
        size="md"
        icon={entry.kindMark.botIcon ? Bot : undefined}
      />
      {entry.kindMark.groupBadge ? (
        <UsersRound
          aria-hidden
          className="text-muted-foreground absolute -right-1 -bottom-0.5 size-3.5 rounded-full bg-background"
        />
      ) : null}
      {dot ? (
        <span
          aria-hidden
          data-testid={`conversation-dot-${entry.id}`}
          className={cn(
            "absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2",
            active ? "ring-white" : "ring-background",
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
        role="img"
        aria-label={t("chat.status.failed")}
        data-testid={`conversation-sendstate-${entry.id}`}
        className="size-3.5 shrink-0 text-destructive"
      />
    ) : entry.sendState === "pending" ? (
      <Clock3
        role="img"
        aria-label={t("chat.status.pending")}
        data-testid={`conversation-sendstate-${entry.id}`}
        className={cn("size-3.5 shrink-0", active ? "text-white/80" : "text-muted-foreground")}
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
          // WX1：行通栏直角（选中绿条贴满列表宽度，微信桌面同款）
          "flex w-full items-start gap-2.5 px-3 py-2.5 text-left transition-colors",
          active ? "bg-primary text-white" : "hover:bg-wx-hover",
        )}
      >
        <RowAvatar entry={entry} active={active} />
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="flex items-center justify-between gap-2">
            <span className="truncate text-sm font-medium">{entry.title}</span>
            {showTime ? (
              <time
                dateTime={new Date(entry.lastTsMs).toISOString()}
                className={cn(
                  "shrink-0 text-[11px]",
                  active ? "text-white/75" : "text-muted-foreground",
                )}
              >
                {formatConversationTime(entry.lastTsMs, locale)}
              </time>
            ) : null}
          </span>
          {entry.subtitle ? (
            <span
              className={cn(
                "truncate text-xs",
                active ? "text-white/80" : "text-muted-foreground",
              )}
            >
              {entry.subtitle}
            </span>
          ) : null}
          <span className="flex items-center gap-1 text-xs">
            {entry.statusBadge ? (
              <Badge variant={entry.statusBadge.tone} className="px-1 py-0 text-[10px]">
                {entry.statusBadge.label}
              </Badge>
            ) : null}
            <span
              className={cn(
                "min-w-0 flex-1 truncate",
                active ? "text-white/80" : "text-muted-foreground",
              )}
            >
              {entry.lastPreview ?? ""}
            </span>
            {sendStateIcon}
            {entry.unread > 0 ? (
              <span
                data-testid={`conversation-unread-${entry.id}`}
                aria-label={t("chat.unread.aria", { count: entry.unread })}
                className={cn(
                  "ml-auto inline-flex min-w-4 shrink-0 items-center justify-center rounded-full px-1 text-[10px] leading-4 font-medium text-white",
                  active ? "bg-white/30" : "bg-wx-badge",
                )}
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
