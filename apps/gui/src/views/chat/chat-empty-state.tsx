import { MessageSquare, UserRoundPlus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/views/shared/empty-state";

// F01（UX 审计 20260907）：主区空态不再断头。列表无可选会话（无好友/群
// 条目）时提供双 CTA：「添加好友」走 #/contacts?add= 预填契约（消费端
// UX-E；新用户无 peerId，传空值仅拉起添加弹窗），「去通讯录」直达列表。
// 文案随列表真实状态切换：有可选条目维持原文案；否则按「等待中邀请 /
// 仅本机 agent / 全空」三态措辞。
export interface ChatEmptyStateProps {
  friendCount: number;
  groupCount: number;
  agentCount: number;
  a2aCount: number;
  pendingInviteCount: number;
}

export function ChatEmptyState({
  friendCount,
  groupCount,
  agentCount,
  a2aCount,
  pendingInviteCount,
}: ChatEmptyStateProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  if (friendCount + groupCount + a2aCount > 0) {
    return (
      <EmptyState
        className="max-w-none flex-1"
        icon={MessageSquare}
        title={t("chat.conversations.empty")}
        description={t("chat.conversations.emptyHint")}
      />
    );
  }
  const hint =
    pendingInviteCount > 0
      ? t("chat.empty.hintInviting")
      : agentCount > 0 || a2aCount > 0
        ? t("chat.empty.hintAgentOnly")
        : t("chat.empty.hintListEmpty");
  return (
    <EmptyState
      className="max-w-none flex-1"
      icon={MessageSquare}
      title={t("chat.empty.pendingTitle")}
      description={hint}
      action={
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            data-testid="chat-empty-add-friend"
            onClick={() => navigate({ pathname: "/contacts", search: "?add=" })}
          >
            <UserRoundPlus aria-hidden />
            {t("chat.empty.addFriendCta")}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            data-testid="chat-empty-go-contacts"
            onClick={() => navigate("/contacts")}
          >
            {t("chat.empty.goContactsCta")}
          </Button>
        </div>
      }
    />
  );
}
