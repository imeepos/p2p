import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { UserRoundPlus } from "lucide-react";

import { CopyButton } from "@/components/monitor/copy-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { MAX_NICKNAME_CHARS } from "@/lib/chat-limits";
import { formatTime } from "@/lib/format";
import type { FriendInviteJson } from "@/lib/ipc-types";
import { shortPeerId } from "@/lib/peer-name";
import { useChatStore } from "@/stores/chat-store";
import type { Locale } from "@/i18n";
import { EmptyState } from "@/views/shared/empty-state";
import { nicknameCharCount } from "@/views/contacts/chat-friend-rules";

// 好友邀请列表（IMC3 需求 2）：方向/状态徽章/时间/备注；in 向待处理行内
// 同意（沿用通讯录既有备注昵称口径：trim 后按字符数 ≤64）/拒绝；行点击跳
// 好友聊天；操作失败原文上浮。状态语义对齐 invite-inbox：列表本身即待处理集。
// F08：out 向待处理行补「撤回」，与通讯录同卡同源（同 store action
// cancelInvite + 同 i18n 标签 contacts.friends.cancelInvite），失败原文上浮。
export function FriendInviteSection() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const navigate = useNavigate();
  const invites = useChatStore((s) => s.invites);
  const acceptInvite = useChatStore((s) => s.acceptInvite);
  const rejectInvite = useChatStore((s) => s.rejectInvite);
  const cancelInvite = useChatStore((s) => s.cancelInvite);
  const [nicknames, setNicknames] = useState<Record<string, string>>({});
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});

  const rows = [...invites].sort((a, b) => b.tsMs - a.tsMs);

  const withdraw = async (invite: FriendInviteJson) => {
    setRowErrors((prev) => ({ ...prev, [invite.peerId]: "" }));
    try {
      await cancelInvite(invite.peerId);
    } catch (err) {
      console.error("[messages] 好友邀请行内撤回失败", invite.peerId, err);
      const detail = err instanceof Error ? err.message : String(err);
      setRowErrors((prev) => ({
        ...prev,
        [invite.peerId]: t("uxk.messages.withdrawFailed") + detail,
      }));
    }
  };

  const accept = async (invite: FriendInviteJson) => {
    const nickname = (nicknames[invite.peerId] ?? "").trim();
    setRowErrors((prev) => ({ ...prev, [invite.peerId]: "" }));
    if (nicknameCharCount(nickname) > MAX_NICKNAME_CHARS) {
      setRowErrors((prev) => ({
        ...prev,
        [invite.peerId]: t("contacts.inviteInbox.nicknameTooLong"),
      }));
      return;
    }
    try {
      await acceptInvite(invite.peerId, nickname);
    } catch (err) {
      console.error("[messages] 好友邀请行内同意失败", invite.peerId, err);
      const detail = err instanceof Error ? err.message : String(err);
      setRowErrors((prev) => ({
        ...prev,
        [invite.peerId]: t("contacts.inviteInbox.acceptFailed") + detail,
      }));
    }
  };

  const reject = async (invite: FriendInviteJson) => {
    setRowErrors((prev) => ({ ...prev, [invite.peerId]: "" }));
    try {
      await rejectInvite(invite.peerId);
    } catch (err) {
      console.error("[messages] 好友邀请行内拒绝失败", invite.peerId, err);
      const detail = err instanceof Error ? err.message : String(err);
      setRowErrors((prev) => ({
        ...prev,
        [invite.peerId]: t("contacts.inviteInbox.rejectFailed") + detail,
      }));
    }
  };

  return (
    <section data-testid="messages-friend-section" className="flex flex-col gap-2">
      <h2 className="text-sm font-semibold">{t("messages.section.friends")}</h2>
      {rows.length === 0 ? (
        <EmptyState icon={UserRoundPlus} title={t("messages.empty.friends")} />
      ) : null}
      <div className="flex flex-col gap-2">
        {rows.map((invite) => {
          const incoming = invite.direction === "in";
          return (
            <div
              key={invite.peerId + String(invite.tsMs)}
              data-testid={"messages-friend-row-" + invite.peerId}
              className="bg-card ring-border ring-1 hover:ring-ring cursor-pointer rounded-lg p-3 transition-shadow"
              onClick={() => navigate("/chat?peer=" + invite.peerId)}
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs">
                  {t(incoming ? "messages.direction.in" : "messages.direction.out")}
                </span>
                <span className="text-sm font-medium">
                  {invite.nickname || shortPeerId(invite.peerId)}
                </span>
                <span className="ml-auto flex items-center gap-2">
                  {incoming ? (
                    <>
                      <Input
                        className="h-8 w-40 text-xs"
                        value={nicknames[invite.peerId] ?? ""}
                        onChange={(e) =>
                          setNicknames((prev) => ({
                            ...prev,
                            [invite.peerId]: e.target.value,
                          }))
                        }
                        placeholder={t("contacts.inviteInbox.nicknameLabel")}
                        aria-label={t("contacts.inviteInbox.nicknameLabel")}
                        data-testid={"messages-friend-nickname-" + invite.peerId}
                        onClick={(e) => e.stopPropagation()}
                        autoComplete="off"
                      />
                      <Button
                        type="button"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          void accept(invite);
                        }}
                        data-testid={"messages-friend-accept-" + invite.peerId}
                      >
                        {t("messages.action.accept")}
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        onClick={(e) => {
                          e.stopPropagation();
                          void reject(invite);
                        }}
                        data-testid={"messages-friend-reject-" + invite.peerId}
                      >
                        {t("messages.action.reject")}
                      </Button>
                    </>
                  ) : (
                    // F08：发出的邀请卡与通讯录同卡同源补撤回
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={(e) => {
                        e.stopPropagation();
                        void withdraw(invite);
                      }}
                      data-testid={"messages-friend-withdraw-" + invite.peerId}
                    >
                      {t("contacts.friends.cancelInvite")}
                    </Button>
                  )}
                  <span className="bg-secondary text-secondary-foreground rounded-full px-2 py-0.5 text-xs font-medium">
                    {t("messages.state.pending")}
                  </span>
                  <time className="text-muted-foreground text-xs">
                    {formatTime(invite.tsMs, locale)}
                  </time>
                </span>
              </div>
              <p className="text-muted-foreground mt-1 flex items-center gap-1 font-mono text-xs">
                <span title={invite.peerId}>{shortPeerId(invite.peerId)}</span>
                <span onClick={(e) => e.stopPropagation()}>
                  <CopyButton value={invite.peerId} className="size-6" />
                </span>
              </p>
              {invite.note ? (
                <p className="text-foreground/80 mt-1 text-xs">
                  {t("chat.groupInvite.noteLabel", { note: invite.note })}
                </p>
              ) : null}
              {rowErrors[invite.peerId] ? (
                <p className="text-destructive mt-1 text-xs" role="alert">
                  {rowErrors[invite.peerId]}
                </p>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}
