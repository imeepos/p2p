import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { EntityMultiSelect, shortPeerId, type PickerOption } from "@/components/picker";
import { MAX_GROUP_MEMBERS } from "@/lib/chat-limits";
import { useGroupStore } from "@/stores/group-store";

import { groupDisplayName } from "./group-names";

interface GroupInvitePickerProps {
  group: { groupId: string; name: string; members: string[] };
  onDone: () => void;
}

// 邀请好友选择面（F23 统一多选选择器）：好友簿减在群成员为候选；
// 即时搜索 + 已选区置顶 + 已选计数；触及群成员上限用现有文案口径就地提示。
// 前置校验与 mock/后端同口径（非空、≤32），后端拒绝原文展示不吞。
export function GroupInvitePicker({ group, onDone }: GroupInvitePickerProps) {
  const { t } = useTranslation();
  const friends = useGroupStore((s) => s.friends);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const invite = useGroupStore((s) => s.invite);
  const [selected, setSelected] = useState<string[]>([]);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    void ensureFriends();
  }, [ensureFriends]);

  const candidates = friends.filter((f) => !group.members.includes(f.peerId));
  const options: PickerOption[] = candidates.map((friend) => ({
    value: friend.peerId,
    label: groupDisplayName(friend.peerId, friends),
    hint: shortPeerId(friend.peerId),
  }));
  const overCap = group.members.length + selected.length > MAX_GROUP_MEMBERS;
  const canSubmit = selected.length > 0 && !overCap && !submitting;

  const submit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setCommandError(null);
    try {
      await invite(group.groupId, selected);
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
        <p className="text-muted-foreground text-xs">{t("group.manage.inviteEmpty")}</p>
      ) : (
        <EntityMultiSelect
          options={options}
          selected={selected}
          onChange={setSelected}
          warning={
            overCap
              ? t("group.manage.inviteOverCap", {
                  count: group.members.length + selected.length,
                  max: MAX_GROUP_MEMBERS,
                })
              : null
          }
          warningTestId="group-invite-overcap"
          testId="group-invite"
        />
      )}
      <div className="flex items-center gap-2">
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
        <div
          className="text-destructive flex flex-col gap-0.5 text-xs"
          role="alert"
          data-testid="group-invite-error"
        >
          {/* 前缀与后端原文分行：长错误串不再与标题挤成一行 */}
          <p className="font-medium">{t("group.manage.inviteFailed")}</p>
          <p className="break-all">{commandError}</p>
        </div>
      ) : null}
    </div>
  );
}
