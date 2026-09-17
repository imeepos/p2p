import { useState } from "react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { CommandErrorText } from "@/components/feedback/command-error";
import { toastError } from "@/components/feedback/toast";
import { Input } from "@/components/ui/input";
import { useChatStore } from "@/stores/chat-store";
import { MAX_NICKNAME_CHARS } from "@/lib/chat-limits";
import { errorText } from "@/views/shared/form-flow";
import { initialOf } from "@/lib/conversation-entry";
import { nicknameCharCount } from "./chat-friend-rules";
import { CONTACT_ROW_CLS, ContactAvatar } from "./contact-avatar";
import { useContactsPane } from "./contacts-sections";
import { TreeSection } from "./contacts-tree";

// 「新的朋友」节（§3.2 加好友 in 向，双栏改版）：默认折叠，红点徽标计数，
// 展开后逐条接受（填备注昵称，同昵称口径 trim ≤64）/拒绝；失败原文上浮
// 不静默。行点选联动右栏邀请资料卡。
export function InviteInbox() {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const invites = useChatStore((s) => s.invites) ?? [];
  const acceptInvite = useChatStore((s) => s.acceptInvite);
  const rejectInvite = useChatStore((s) => s.rejectInvite);
  const [expanded, setExpanded] = useState(false);
  const [nicknames, setNicknames] = useState<Record<string, string>>({});
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});

  const incoming = invites.filter((i) => i.direction === "in");

  const accept = async (peerId: string) => {
    const nickname = (nicknames[peerId] ?? "").trim();
    if (nicknameCharCount(nickname) > MAX_NICKNAME_CHARS) {
      setRowErrors((prev) => ({
        ...prev,
        [peerId]: t("contacts.inviteInbox.nicknameTooLong"),
      }));
      // 校验失败同样呈 fail 态：action 正常 resolve 会被 AsyncButton 记成功
      throw new Error("nickname validation failed");
    }
    setRowErrors((prev) => ({ ...prev, [peerId]: "" }));
    try {
      await acceptInvite(peerId, nickname);
    } catch (err) {
      console.error("[contacts] 接受邀请失败", peerId, err);
      setRowErrors((prev) => ({
        ...prev,
        [peerId]:
          t("contacts.inviteInbox.acceptFailed") +
          (err instanceof Error ? err.message : String(err)),
      }));
      // 抛给 AsyncButton 呈现 fail 态，避免失败亮成功勾（messages 组同口径）
      throw err;
    }
  };

  // P2#5 拒绝与接受对称走 AsyncButton：失败 toast 报错，不写底部共享 error。
  const reject = (peerId: string) => rejectInvite(peerId);

  return (
    <TreeSection
      id="invites"
      wrapperTestId="contacts-invite-inbox"
      title={t("contacts.inviteInbox.treeTitle")}
      expanded={expanded}
      onToggle={() => setExpanded((v) => !v)}
      toggleTestId="contacts-invite-toggle"
      badge={
        incoming.length > 0 ? (
          <span
            aria-label={t("contacts.inviteBadge.aria", { count: incoming.length })}
            data-testid="contacts-invite-dot"
            className="bg-destructive inline-flex min-w-5 items-center justify-center rounded-full px-1 text-[10px] leading-5 font-medium text-white"
          >
            {incoming.length}
          </span>
        ) : null
      }
    >
      {incoming.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm">{t("contacts.inviteInbox.empty")}</p>
      ) : (
        incoming.map((invite) => (
          <div
            key={invite.peerId}
            className="flex flex-col gap-1.5 rounded-md px-1 py-1"
            data-testid={"contacts-invite-in-" + invite.peerId}
          >
            <div className={CONTACT_ROW_CLS}>
              <button
                type="button"
                className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md text-left"
                onClick={() =>
                  pane.select({ kind: "invite", peerId: invite.peerId, direction: "in" })
                }
              >
                <ContactAvatar initial={initialOf(invite.nickname)} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium">{invite.nickname}</span>
                  <span className="text-muted-foreground block truncate text-xs">
                    {t("contacts.inviteInbox.pendingIn")}
                  </span>
                </span>
              </button>
              <AsyncButton
                type="button"
                size="sm"
                className="h-7 px-2 text-xs"
                action={() => accept(invite.peerId)}
                data-testid={"contacts-invite-accept-" + invite.peerId}
              >
                {t("contacts.inviteInbox.accept")}
              </AsyncButton>
              <AsyncButton
                type="button"
                size="sm"
                variant="outline"
                className="h-7 px-2 text-xs"
                action={() => reject(invite.peerId)}
                onError={(err) => {
                  console.error("[contacts] 拒绝邀请失败", invite.peerId, err);
                  toastError(t("contacts.inviteInbox.rejectFailedToast"), {
                    description: errorText(err),
                    context: "chat_invite_reject",
                  });
                }}
                data-testid={"contacts-invite-reject-" + invite.peerId}
              >
                {t("contacts.inviteInbox.reject")}
              </AsyncButton>
            </div>
            <Input
              className="h-8 text-xs"
              value={nicknames[invite.peerId] ?? ""}
              onChange={(e) =>
                setNicknames((prev) => ({ ...prev, [invite.peerId]: e.target.value }))
              }
              placeholder={t("contacts.inviteInbox.nicknameLabel")}
              aria-label={t("contacts.inviteInbox.nicknameLabel")}
              data-testid={"contacts-invite-nickname-" + invite.peerId}
              autoComplete="off"
            />
            {rowErrors[invite.peerId] ? (
              <CommandErrorText
                message={rowErrors[invite.peerId]}
                testId={"contacts-invite-row-error-" + invite.peerId}
              />
            ) : null}
          </div>
        ))
      )}
    </TreeSection>
  );
}
