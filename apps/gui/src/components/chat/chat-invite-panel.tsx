import { useChatStore } from "@/stores/chat-store";
import { useTranslation } from "react-i18next";
import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import { UserCheckIcon } from "lucide-react";

// 待处理好友邀请面板（邀请制加好友，契约 v9 §12.4）：来邀可同意/拒绝，
// 已发邀请展示等待状态。数据来自 chat-store.invites（chat_invite 事件驱动刷新）。
export function ChatInvitePanel() {
  const { t } = useTranslation();
  const invites = useChatStore((s) => s.invites) ?? [];
  const acceptInvite = useChatStore((s) => s.acceptInvite);
  const rejectInvite = useChatStore((s) => s.rejectInvite);
  const incoming = invites.filter((i) => i.direction === "in");
  const outgoing = invites.filter((i) => i.direction === "out");

  if (incoming.length === 0 && outgoing.length === 0) return null;

  return (
    <div
      className="flex flex-col gap-2 border-b p-3"
      data-testid="chat-invite-panel"
    >
      {incoming.map((invite) => (
        <div
          key={"in:" + invite.peerId}
          className="flex flex-col gap-2 rounded-md border p-2"
          data-testid={"chat-invite-in-" + invite.peerId}
        >
          <p className="text-sm font-medium">
            {t("chat.invite.incoming", { name: invite.nickname })}
          </p>
          <p className="font-mono text-xs text-muted-foreground">
            {invite.peerId}
          </p>
          <div className="flex gap-2">
            <AsyncButton
              type="button"
              size="sm"
              action={() => acceptInvite(invite.peerId, "")}
              onError={(error: unknown) =>
                console.error("[chat] 同意邀请失败", error)
              }
              data-testid={"chat-invite-accept-" + invite.peerId}
            >
              {t("chat.invite.accept")}
            </AsyncButton>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => void rejectInvite(invite.peerId)}
              data-testid={"chat-invite-reject-" + invite.peerId}
            >
              {t("chat.invite.reject")}
            </Button>
          </div>
        </div>
      ))}
      {outgoing.map((invite) => (
        <p
          key={"out:" + invite.peerId}
          className="text-xs text-muted-foreground"
          data-testid={"chat-invite-out-" + invite.peerId}
        >
          {t("chat.invite.outgoing", { name: invite.nickname })}
        </p>
      ))}
      {incoming.length === 0 ? (
        <p className="flex items-center gap-1 text-xs text-muted-foreground">
          <UserCheckIcon aria-hidden className="size-3" />
          {t("chat.invite.empty")}
        </p>
      ) : null}
    </div>
  );
}
