import { Hourglass } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { initialOf } from "@/lib/conversation-entry";
import type { PendingInviteItem } from "@/views/chat/use-pending-invites";

// F17（UX 审计 20260907）：会话列表的「等待对方同意」置灰占位行。
// 邀请未获同意前不存在可打开的会话，行不可点击（aria-disabled），
// 仅呈现标题与等待状态徽标；同意后的真条目由好友/群簿驱动。
export function InvitePlaceholderRow({ item }: { item: PendingInviteItem }) {
  const { t } = useTranslation();
  const label = t("chat.pendingInviteBadge");
  return (
    <li>
      <div
        aria-disabled="true"
        data-testid={`conversation-invite-${item.kind}-${item.id}`}
        title={`${item.title}：${label}`}
        className="flex w-full items-start gap-2 px-3 py-2 text-left text-sm opacity-60"
      >
        <span
          aria-hidden
          className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-full text-sm font-medium"
        >
          {initialOf(item.title)}
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate font-medium">{item.title}</span>
          <span className="text-muted-foreground flex items-center gap-1 text-xs">
            <Hourglass aria-hidden className="size-3.5 shrink-0" />
            <Badge variant="outline" className="px-1 py-0 text-[10px]">
              {label}
            </Badge>
          </span>
        </span>
      </div>
    </li>
  );
}
