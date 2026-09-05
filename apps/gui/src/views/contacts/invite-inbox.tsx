import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDownIcon, ChevronUpIcon, UserRoundPlusIcon } from "lucide-react";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useChatStore } from "@/stores/chat-store";
import { MAX_NICKNAME_CHARS } from "@/lib/chat-limits";
import { nicknameCharCount } from "./chat-friend-rules";

// 待处理好友邀请收件箱（§3.2 加好友 in 向）：页顶红点徽标计数，展开后
// 逐条接受（填备注昵称，同昵称口径 trim ≤64）/拒绝；失败原文上浮不静默。
export function InviteInbox() {
  const { t } = useTranslation();
  const invites = useChatStore((s) => s.invites) ?? [];
  const acceptInvite = useChatStore((s) => s.acceptInvite);
  const rejectInvite = useChatStore((s) => s.rejectInvite);
  const [expanded, setExpanded] = useState(false);
  const [nicknames, setNicknames] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  const incoming = invites.filter((i) => i.direction === "in");
  if (incoming.length === 0) return null;

  const accept = async (peerId: string) => {
    const nickname = (nicknames[peerId] ?? "").trim();
    if (nicknameCharCount(nickname) > MAX_NICKNAME_CHARS) {
      setError(t("contacts.inviteInbox.nicknameTooLong"));
      return;
    }
    setError(null);
    try {
      await acceptInvite(peerId, nickname);
    } catch (err) {
      console.error("[contacts] 接受邀请失败", peerId, err);
      setError(
        t("contacts.inviteInbox.acceptFailed") +
          (err instanceof Error ? err.message : String(err)),
      );
    }
  };

  const reject = async (peerId: string) => {
    setError(null);
    try {
      await rejectInvite(peerId);
    } catch (err) {
      console.error("[contacts] 拒绝邀请失败", peerId, err);
      setError(
        t("contacts.inviteInbox.rejectFailed") +
          (err instanceof Error ? err.message : String(err)),
      );
    }
  };

  return (
    <div
      className="bg-card ring-border ring-1 flex flex-col gap-2 rounded-lg p-3"
      data-testid="contacts-invite-inbox"
    >
      <button
        type="button"
        className="flex w-fit items-center gap-2"
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        data-testid="contacts-invite-toggle"
      >
        <UserRoundPlusIcon aria-hidden className="size-4" />
        <span className="text-sm font-medium">
          {t("contacts.inviteInbox.title")}（{incoming.length}）
        </span>
        <span
          aria-label={t("contacts.inviteBadge.aria", { count: incoming.length })}
          data-testid="contacts-invite-dot"
          className="bg-destructive inline-flex min-w-5 items-center justify-center rounded-full px-1 text-[10px] leading-5 font-medium text-white"
        >
          {incoming.length}
        </span>
        {expanded ? (
          <ChevronUpIcon aria-hidden className="size-4" />
        ) : (
          <ChevronDownIcon aria-hidden className="size-4" />
        )}
      </button>
      {expanded ? (
        <div className="flex flex-col gap-2">
          {incoming.map((invite) => (
            <div
              key={invite.peerId}
              className="flex flex-col gap-2 rounded-md border p-2"
              data-testid={"contacts-invite-in-" + invite.peerId}
            >
              <p className="text-sm font-medium">
                {t("chat.invite.incoming", { name: invite.nickname })}
              </p>
              <p className="text-muted-foreground font-mono text-xs">{invite.peerId}</p>
              <div className="flex flex-wrap items-center gap-2">
                <Input
                  className="h-8 w-44 text-xs"
                  value={nicknames[invite.peerId] ?? ""}
                  onChange={(e) =>
                    setNicknames((prev) => ({ ...prev, [invite.peerId]: e.target.value }))
                  }
                  placeholder={t("contacts.inviteInbox.nicknameLabel")}
                  aria-label={t("contacts.inviteInbox.nicknameLabel")}
                  data-testid={"contacts-invite-nickname-" + invite.peerId}
                  autoComplete="off"
                />
                <AsyncButton
                  type="button"
                  size="sm"
                  action={() => accept(invite.peerId)}
                  data-testid={"contacts-invite-accept-" + invite.peerId}
                >
                  {t("contacts.inviteInbox.accept")}
                </AsyncButton>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => void reject(invite.peerId)}
                  data-testid={"contacts-invite-reject-" + invite.peerId}
                >
                  {t("contacts.inviteInbox.reject")}
                </Button>
              </div>
            </div>
          ))}
          {error ? (
            <p className="text-destructive text-xs" role="alert" data-testid="contacts-invite-error">
              {error}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
