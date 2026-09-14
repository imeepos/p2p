import { BellOff, Bot, CircleAlert, Clock3, UsersRound } from "lucide-react";
import type { MouseEvent as ReactMouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { AvatarBox } from "@/components/chat/avatar-box";
import { Badge } from "@/components/ui/badge";
import { formatConversationTime } from "@/lib/format";
import { formatUnreadCount } from "@/lib/conversation-entry";
import type { ConversationEntry } from "@/lib/conversation-entry";
import type { Locale } from "@/i18n";
import { cn } from "@/lib/utils";

// WX1 微信风格会话行：方形头像 + 名称/时间行 + 预览行；选中态 accent 弱填充 +
// 标题加重（uix-spec #8，替换原微信绿满宽白字高噪选中）。
// 未读红点角标（≥100 显 99+，§2.3）；failed/error 显红色感叹角标，pending 显
// 时钟图标；头像右上在线状态点保留（功能位）。

const DOT_CLASS: Record<string, string> = {
  green: "bg-success",
  yellow: "bg-warning",
  red: "bg-destructive",
};

// uix-spec §1 #8：选中态与 hover 同为 accent 弱填充（靠字重/徽章区分），
// 替换微信绿满宽白字的高噪选中样式
function RowAvatar({ entry }: { entry: ConversationEntry }) {
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
  /** 右键菜单入口（阻止默认浏览器菜单后上抛坐标）；未挂菜单的列表可缺省 */
  onContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  /** 本机免打扰（右键菜单产物）：预览行显静音铃 */
  muted?: boolean;
}

export function ConversationRow({ entry, active, onSelect, onContextMenu, muted }: ConversationRowProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const showTime = entry.lastTsMs > 0;
  const sendStateIcon =
    entry.sendState === "failed" || entry.sendState === "error" ? (
      <CircleAlert
        role="img"
        aria-label={t("chat.status.failed")}
        data-testid={`conversation-sendstate-${entry.id}`}
        className="text-destructive size-3.5 shrink-0"
      />
    ) : entry.sendState === "pending" ? (
      <Clock3
        role="img"
        aria-label={t("chat.status.pending")}
        data-testid={`conversation-sendstate-${entry.id}`}
        className="text-muted-foreground size-3.5 shrink-0"
      />
    ) : null;
  return (
    <li>
      <button
        type="button"
        onClick={() => onSelect(entry)}
        onContextMenu={(event) => onContextMenu?.(event)}
        aria-current={active || undefined}
        data-testid={`conversation-row-${entry.kind}-${entry.id}`}
        className={cn(
          // 行通栏直角；选中/悬停同一 accent 弱填充（uix-spec §1 #7/#8）
          "flex w-full items-start gap-2.5 px-3 py-2.5 text-left transition-colors",
          active ? "bg-accent" : "hover:bg-wx-hover",
        )}
      >
        <RowAvatar entry={entry} />
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="flex items-center justify-between gap-2">
            <span className={cn("truncate text-sm", active ? "font-semibold" : "font-medium")}>
              {entry.title}
            </span>
            {showTime ? (
              <time
                dateTime={new Date(entry.lastTsMs).toISOString()}
                className="text-muted-foreground shrink-0 text-[11px]"
              >
                {formatConversationTime(entry.lastTsMs, locale)}
              </time>
            ) : null}
          </span>
          {entry.subtitle ? (
            <span className="text-muted-foreground truncate text-xs">{entry.subtitle}</span>
          ) : null}
          <span className="flex items-center gap-1 text-xs">
            {entry.statusBadge ? (
              <Badge variant={entry.statusBadge.tone} className="px-1 py-0 text-[10px]">
                {entry.statusBadge.label}
              </Badge>
            ) : null}
            <span className="text-muted-foreground min-w-0 flex-1 truncate">
              {entry.lastPreview ?? ""}
            </span>
            {muted ? (
              <BellOff
                role="img"
                aria-label={t("chat.conversations.mutedAria")}
                data-testid={`conversation-muted-${entry.id}`}
                className="text-muted-foreground size-3.5 shrink-0"
              />
            ) : null}
            {sendStateIcon}
            {entry.unread > 0 ? (
              <span
                data-testid={`conversation-unread-${entry.id}`}
                aria-label={t("chat.unread.aria", { count: entry.unread })}
                className="ml-auto inline-flex min-w-4 shrink-0 items-center justify-center rounded-full bg-wx-badge px-1 text-[10px] leading-4 font-medium text-white"
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
