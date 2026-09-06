import { BellIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { selectPendingInviteBadgeCount } from "@/stores/chat-group-invite-slice";
import { useChatStore } from "@/stores/chat-store";

// 顶栏消息中心入口（IMC3 需求 2）：铃铛 + 未读徽标。徽标数 =
// 入群邀请 in 向 pending + 好友邀请 in 向（chat-store 双切片求和 selector，
// 返回原始 number 规避快照引用漂移）。rail 一级菜单零改动。
export function NotificationBell() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const count = useChatStore(selectPendingInviteBadgeCount);

  return (
    <Button
      variant="ghost"
      size="icon"
      className="relative"
      aria-label={t("messages.title")}
      title={t("messages.title")}
      data-testid="notification-bell"
      onClick={() => navigate("/messages")}
    >
      <BellIcon className="size-4" />
      {count > 0 ? (
        <span
          data-testid="notification-badge"
          aria-label={t("messages.badgeAria", { count })}
          className="bg-destructive absolute -top-1 -right-1 inline-flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] leading-4 font-medium text-white"
        >
          {count > 99 ? "99+" : count}
        </span>
      ) : null}
    </Button>
  );
}
