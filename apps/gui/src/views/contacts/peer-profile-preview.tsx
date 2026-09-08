import { useTranslation } from "react-i18next";

import { isValidPeerId } from "@/lib/chat-limits";
import type { PeerProfileJson } from "@/lib/ipc-types";
import { usePeerProfileStore } from "@/stores/peer-profile-store";

import { ContactAvatar } from "./contact-avatar";

// 对端自报资料预览（添加好友弹窗内）：头像 + 节点名 + 简介；资料不可得
// （离线/未设置/对端旧版）静默降级为一行提示，不阻塞手工填写。
export function PeerProfilePreview({ peerId }: { peerId: string }) {
  const { t } = useTranslation();
  const profile = usePeerProfileStore((s) => s.profiles[peerId]);
  if (!isValidPeerId(peerId)) return null;
  if (!profile) {
    return (
      <p className="text-muted-foreground text-xs" data-testid="friend-add-profile-empty">
        {t("contacts.addFriend.profileUnavailable")}
      </p>
    );
  }
  return <ProfileCard profile={profile} peerId={peerId} />;
}

function ProfileCard({ profile, peerId }: { profile: PeerProfileJson; peerId: string }) {
  const { t } = useTranslation();
  const name = profile.name || t("contacts.addFriend.profileUnnamed");
  return (
    <div
      className="bg-muted/40 border-border flex items-start gap-3 rounded-md border p-2.5"
      data-testid="friend-add-profile-card"
    >
      <ContactAvatar initial={name.slice(0, 1)} src={profile.avatar} className="size-10 text-sm" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{name}</p>
        {profile.description ? (
          <p className="text-muted-foreground line-clamp-2 text-xs">{profile.description}</p>
        ) : null}
      </div>
      <span className="text-muted-foreground shrink-0 text-[10px]">
        {t("contacts.addFriend.profileSource", { peer: peerId.slice(0, 6) })}
      </span>
    </div>
  );
}
