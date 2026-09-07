import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";

import { AvatarBox } from "@/components/chat/avatar-box";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { MENU_ENTRIES } from "@/config/menu.def";
import { useIncomingInviteCount } from "@/hooks/use-incoming-invites";
import { useUnreadTotal } from "@/hooks/use-unread-total";
import { formatUnreadCount } from "@/lib/conversation-entry";
import { cn } from "@/lib/utils";
import { selectPendingInviteBadgeCount } from "@/stores/chat-group-invite-slice";
import { useChatStore } from "@/stores/chat-store";
import { useProfileStore } from "@/stores/profile-store";

// 侧栏 rail：底色与图标用语义 --sidebar/--muted 令牌，随亮暗主题自适应
// （不再固定深灰底）。角标红点（--wx-badge）。聊天入口附未读合计、
// 通讯录附好友邀请、消息中心附待处理邀请（角标来源一致）。
function SelfAvatar() {
  const { t } = useTranslation();
  const profile = useProfileStore((s) => s.profile);
  const loadProfile = useProfileStore((s) => s.load);
  const label = profile.name.trim() || "P2P";

  useEffect(() => {
    // 失败路径已入 profile-store loadError + console，此处吞掉即可
    if (!useProfileStore.getState().loaded) {
      loadProfile().catch(() => {});
    }
  }, [loadProfile]);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <NavLink
          to="/settings"
          aria-label={t("settings.title")}
          className="focus-visible:ring-ring/50 mt-2 mb-1 flex items-center justify-center rounded-md focus-visible:ring-[3px] focus-visible:outline-none"
        >
          <AvatarBox
            label={label}
            src={profile.avatar}
            seed="self"
            size="lg"
            className="shadow-sm"
          />
        </NavLink>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

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
              "focus-visible:ring-ring/50 relative flex size-10 items-center justify-center rounded-md text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-[3px] focus-visible:outline-none",
              isActive && "text-primary hover:text-primary",
            )
          }
        >
          <Icon className="size-5 shrink-0" aria-hidden />
          {badge !== undefined && badge > 0 ? (
            <span
              data-testid={`rail-badge-${path}`}
              aria-label={badgeLabel}
              className="bg-wx-badge absolute top-0.5 right-0.5 inline-flex min-w-4 items-center justify-center rounded-full px-1 text-[9px] leading-4 font-medium text-white"
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
  const pendingInvites = useChatStore(selectPendingInviteBadgeCount);
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
    // F15：消息中心角标与顶栏铃铛同源 selector（两类 in 向 pending 之和）
    if (path === "/messages") {
      return { count: pendingInvites, label: t("messages.badgeAria", { count: pendingInvites }) };
    }
    return undefined;
  };
  return (
    <aside className="bg-sidebar flex h-full w-14 shrink-0 flex-col items-center border-r border-sidebar-border">
      <SelfAvatar />
      <nav className="flex w-full flex-1 flex-col items-center gap-1.5 px-1.5 pb-2">
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
