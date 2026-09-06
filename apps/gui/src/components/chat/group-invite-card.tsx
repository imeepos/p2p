import { useTranslation } from "react-i18next";

import { toastInfo } from "@/components/feedback/toast";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";

// 入群邀请卡片（IMC3 需求 1）：1:1 消息流 kind=groupInvite 专用渲染。
// - 自己发出的卡：方向徽标（发出的）+ 状态徽章（待处理/已同意/已拒绝）。
// - 他人发来的待处理卡：整卡可点，打开入群确认弹框；已处理卡点击给提示态。
// - 邀请簿未同步（invite=null）按待处理呈现，不臆造终态。

const STATE_KEYS = {
  pending: "messages.state.pending",
  accepted: "messages.state.accepted",
  rejected: "messages.state.rejected",
} as const;

interface GroupInviteCardProps {
  message: ChatMessageJson;
  invite: GroupInviteJson | null;
  onOpenConfirm?: (invite: GroupInviteJson | null) => void;
}

export function GroupInviteStateBadge({ state }: { state: GroupInviteJson["state"] }) {
  const { t } = useTranslation();
  const tone =
    state === "pending"
      ? "bg-secondary text-secondary-foreground"
      : state === "accepted"
        ? "bg-primary/15 text-primary"
        : "bg-muted text-muted-foreground";
  return (
    <span
      data-testid="group-invite-state"
      className={cn("rounded-full px-2 py-0.5 text-xs font-medium", tone)}
    >
      {t(STATE_KEYS[state])}
    </span>
  );
}

export function GroupInviteCard({ message, invite, onOpenConfirm }: GroupInviteCardProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const isMe = message.sender === "me";
  const body = message.groupInvite;
  const state = invite?.state ?? "pending";
  const clickable = !isMe && state === "pending";

  const clickProcessed = () => {
    toastInfo(
      t(
        state === "accepted"
          ? "chat.groupInvite.processedAccepted"
          : "chat.groupInvite.processedRejected",
      ),
    );
  };

  const metaRow = (
    <div className="text-muted-foreground mt-1 flex items-center gap-2 text-xs">
      <time>{formatTime(message.tsMs, locale)}</time>
      {isMe ? (
        <span data-testid="group-invite-direction" className="rounded-full bg-muted px-2 py-0.5">
          {t("messages.direction.out")}
        </span>
      ) : null}
      <GroupInviteStateBadge state={state} />
    </div>
  );

  const bodyNode = (
    <>
      <p className="text-muted-foreground text-xs">
        {isMe ? t("chat.groupInvite.inviteSent") : t("chat.groupInvite.inviteYou")}
      </p>
      <p className="text-foreground text-sm font-semibold">
        {body?.groupName ?? message.peer.slice(0, 8)}
      </p>
      {!isMe && body ? (
        <p className="text-muted-foreground text-xs">
          {t("chat.groupInvite.inviterLabel", { name: body.inviterNickname })}
        </p>
      ) : null}
      {body?.note ? (
        <p className="text-foreground/80 text-xs">
          {t("chat.groupInvite.noteLabel", { note: body.note })}
        </p>
      ) : null}
      {metaRow}
    </>
  );

  const shell = clickable
    ? "bg-card hover:ring-ring cursor-pointer hover:ring-2"
    : "bg-card";
  return (
    <div
      className={cn("group relative flex w-full", isMe ? "justify-end" : "justify-start")}
      data-message-id={message.id}
    >
      {clickable ? (
        <button
          type="button"
          data-testid="group-invite-card"
          className={cn("max-w-[75%] rounded-lg border p-3 text-left", shell)}
          onClick={() => onOpenConfirm?.(invite)}
        >
          {bodyNode}
        </button>
      ) : (
        <div
          data-testid="group-invite-card"
          className={cn("max-w-[75%] rounded-lg border p-3 text-left", shell)}
          onClick={isMe ? undefined : clickProcessed}
        >
          {bodyNode}
        </div>
      )}
    </div>
  );
}
