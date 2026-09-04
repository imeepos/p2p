import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { MAX_GROUP_MEMBERS } from "@/lib/mock-chat-rules";
import { useGroupStore } from "@/stores/group-store";

import { groupDisplayName } from "./group-names";

interface GroupInvitePickerProps {
  group: { groupId: string; name: string; members: string[] };
  onDone: () => void;
}

// 邀请好友勾选面（设计 §5 邀请）：好友簿减在群成员为候选；
// 前置校验与 mock/后端同口径（非空、≤32），后端拒绝原文展示不吞。
export function GroupInvitePicker({ group, onDone }: GroupInvitePickerProps) {
  const { t } = useTranslation();
  const friends = useGroupStore((s) => s.friends);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const invite = useGroupStore((s) => s.invite);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    void ensureFriends();
  }, [ensureFriends]);

  const candidates = friends.filter((f) => !group.members.includes(f.peerId));
  const overCap = group.members.length + selected.size > MAX_GROUP_MEMBERS;
  const canSubmit = selected.size > 0 && !overCap && !submitting;

  const toggle = (peerId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(peerId)) next.delete(peerId);
      else next.add(peerId);
      return next;
    });
  };

  const submit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setCommandError(null);
    try {
      await invite(group.groupId, [...selected]);
      onDone();
    } catch (error) {
      console.error("[group] 邀请成员失败", error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex flex-col gap-2 rounded-md border p-2" data-testid="group-invite-picker">
      <p className="text-xs font-medium">{t("group.manage.inviteTitle")}</p>
      {candidates.length === 0 ? (
        <p className="text-xs text-muted-foreground">{t("group.manage.inviteEmpty")}</p>
      ) : (
        <div className="scroll-slim flex max-h-40 flex-col overflow-y-auto">
          {candidates.map((friend) => (
            <label
              key={friend.peerId}
              className="hover:bg-accent flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm"
            >
              <input
                type="checkbox"
                className="size-4"
                checked={selected.has(friend.peerId)}
                onChange={() => toggle(friend.peerId)}
                data-testid={`group-invite-${friend.peerId}`}
              />
              <span className="truncate">{groupDisplayName(friend.peerId, friends)}</span>
            </label>
          ))}
        </div>
      )}
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground text-xs">
          {t("group.manage.inviteSelected", { count: selected.size })}
        </span>
        {overCap ? (
          <span className="text-destructive text-xs" data-testid="group-invite-overcap">
            {t("group.manage.inviteOverCap", {
              count: group.members.length + selected.size,
              max: MAX_GROUP_MEMBERS,
            })}
          </span>
        ) : null}
        <Button
          type="button"
          size="sm"
          className="ml-auto"
          onClick={() => void submit()}
          disabled={!canSubmit}
          data-testid="group-invite-submit"
        >
          {t("group.manage.inviteSubmit")}
        </Button>
      </div>
      {commandError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="group-invite-error">
          {t("group.manage.inviteFailed")}
          {commandError}
        </p>
      ) : null}
    </div>
  );
}
