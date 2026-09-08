import { useTranslation } from "react-i18next";
import { CopyButton } from "@/components/feedback/copy-button";
import { initialOf } from "@/lib/conversation-entry";
import { formatDateTime } from "@/lib/format";
import type { FriendInviteJson } from "@/lib/ipc-types";
import type { Locale } from "@/i18n";

import { ContactAvatar } from "./contact-avatar";
import { DetailRow, DetailRows, DetailShell } from "./detail-bits";

// 好友邀请资料卡：头像 + 昵称；ID/状态/来源/添加时间。接受/拒绝/撤回
// 仍留在「新的朋友」行内（逐条处理表单与红点计数挂钩），卡片只读展示。
export function DetailInvite({ invite }: { invite: FriendInviteJson }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  return (
    <DetailShell
      avatar={<ContactAvatar initial={initialOf(invite.nickname)} className="size-16 rounded-lg text-xl" />}
      title={<p className="truncate text-lg font-semibold">{invite.nickname}</p>}
    >
      <DetailRows>
        <DetailRow label={t("contacts.detail.peerId")}>
          <span className="font-mono text-xs break-all">{invite.peerId}</span>
          <CopyButton value={invite.peerId} className="size-5 shrink-0" />
        </DetailRow>
        <DetailRow label={t("contacts.detail.status")}>
          {invite.direction === "in"
            ? t("contacts.detail.pendingIn")
            : t("contacts.friends.pendingOut")}
        </DetailRow>
        <DetailRow label={t("contacts.detail.source")}>{t("contacts.detail.sourceInvite")}</DetailRow>
        <DetailRow label={t("contacts.detail.addTime")}>
          {formatDateTime(invite.tsMs, locale)}
        </DetailRow>
      </DetailRows>
    </DetailShell>
  );
}
