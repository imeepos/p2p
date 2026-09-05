import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { MENU_ENTRIES } from "@/config/menu.def";
import { useIncomingInviteCount } from "@/hooks/use-incoming-invites";
import { useUnreadTotal } from "@/hooks/use-unread-total";
import { formatUnreadCount } from "@/lib/conversation-entry";
import { cn } from "@/lib/utils";

// 窄图标栏（docs/design/app-shell-redesign.md 1.1）：常驻 w-14，仅图标 +
// tooltip + 选中态高亮，不再提供折叠形态；注册序末项（设置）沉底。
// 聊天入口附三来源未读合计角标（§2.3）；通讯录入口附带处理好友邀请
// 角标（§3.2）。顶栏与底部状态栏不在 rail 职责内。
function RailLink({
  path,
  titleKey,
  icon: Icon,
  badge,
  badgeLabel,
}: (typeof MENU_ENTRIES)[number] & { badge?: number; badgeLabel?: string }) {
  const { t } = useTranslation();
  const label = t(titleKey);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <NavLink
          to={path}
          aria-label={label}
          className={({ isActive }) =>
            cn(
              "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-ring/50 relative flex size-9 items-center justify-center rounded-md focus-visible:ring-[3px] focus-visible:outline-none",
              isActive && "bg-sidebar-accent text-sidebar-accent-foreground font-semibold",
            )
          }
        >
          <Icon className="size-4 shrink-0" aria-hidden />
          {badge !== undefined && badge > 0 ? (
            <span
              data-testid={`rail-badge-${path}`}
              aria-label={badgeLabel}
              className="bg-neutral-500 absolute -top-0.5 -right-0.5 inline-flex min-w-3.5 items-center justify-center rounded-full px-1 text-[9px] leading-3.5 font-medium text-white"
            >
              {formatUnreadCount(badge)}
            </span>
          ) : null}
        </NavLink>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

export function IconRail() {
  const { t } = useTranslation();
  const top = MENU_ENTRIES.slice(0, -1);
  const bottom = MENU_ENTRIES[MENU_ENTRIES.length - 1];
  const unreadTotal = useUnreadTotal();
  const incomingInvites = useIncomingInviteCount();
  const badgeOf = (path: string): { count: number; label: string } | undefined => {
    // §2.3 聊天未读合计角标；§3.2 通讯录待处理好友邀请角标
    if (path === "/chat") {
      return { count: unreadTotal, label: t("chat.unread.aria", { count: unreadTotal }) };
    }
    if (path === "/contacts") {
      return {
        count: incomingInvites,
        label: t("contacts.inviteBadge.aria", { count: incomingInvites }),
      };
    }
    return undefined;
  };
  return (
    <aside className="bg-sidebar text-sidebar-foreground flex h-full w-14 flex-col border-r">
      <nav className="flex flex-1 flex-col items-center gap-1 p-2">
        {top.map((entry) => {
          const badge = badgeOf(entry.path);
          return (
            <RailLink
              key={entry.path}
              {...entry}
              badge={badge?.count}
              badgeLabel={badge?.label}
            />
          );
        })}
        {/* 弹性空隙：高频入口（聊天/通讯录/网络）居上，低频设置沉底 */}
        <div className="flex-1" />
        {bottom ? <RailLink {...bottom} /> : null}
      </nav>
    </aside>
  );
}
