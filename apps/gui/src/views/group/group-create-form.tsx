import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { UserRoundPlus } from "lucide-react";

import { EntityMultiSelect, shortPeerId, type PickerOption } from "@/components/picker";
import { CommandErrorText } from "@/components/feedback/command-error";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MAX_GROUP_MEMBERS, MAX_GROUP_NAME_CHARS } from "@/lib/chat-limits";
import type { GroupJson } from "@/lib/ipc-types";
import { ipc } from "@/lib/ipc";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";

import { groupDisplayName } from "./group-names";

interface GroupCreateFormProps {
  /** 建群成功后由宿主关闭弹层 */
  onDone: () => void;
}

// 建群表单（设计 §5）：好友簿选成员 → 命名 → groupCreate。F09 起被
// GroupCreateDialog 与联系人「添加群聊」弹窗共用，两处一跳直达。
// F10：文案说用户语言（1-64 个字），trim 内部规则交给校验不上文案。
// F23：成员列表复用统一多选选择器（即时搜索 + 已选区置顶），与
// group-invite-picker 同一交互口径；上限 32（含本机）。前置校验同 mock/后端。
export function GroupCreateForm({ onDone }: GroupCreateFormProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const friends = useGroupStore((s) => s.friends);
  const friendsLoading = useGroupStore((s) => s.friendsLoading);
  const friendsError = useGroupStore((s) => s.friendsError);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const upsertGroup = useGroupStore((s) => s.upsertGroup);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    void ensureFriends();
  }, [ensureFriends]);

  const memberOptions: PickerOption[] = friends.map((friend) => ({
    value: friend.peerId,
    label: groupDisplayName(friend.peerId, friends),
    hint: shortPeerId(friend.peerId),
  }));
  const trimmedName = name.trim();
  const nameTooLong = Array.from(trimmedName).length > MAX_GROUP_NAME_CHARS;
  const overCap = 1 + selected.length > MAX_GROUP_MEMBERS;
  const canSubmit =
    trimmedName.length > 0 && !nameTooLong && selected.length > 0 && !overCap && !submitting;

  const goAddFriend = () => {
    // 跨卡 URL 契约：#/contacts?add= 挂载即开添加好友弹窗
    onDone();
    navigate("/contacts?add=");
  };

  const submit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setCommandError(null);
    try {
      const group: GroupJson = await ipc.groupCreate(trimmedName, selected);
      upsertGroup(group);
      try {
        await selectGroup(group.groupId);
      } catch (error) {
        // 已建群入列；仅首屏历史加载失败，不回滚建群，留日志信号。
        console.error("[group] 新群历史加载失败", error);
      }
      onDone();
    } catch (error) {
      console.error("[group] 建群失败", error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  // 加载中/失败不落死表单：选择器三态就地呈现；真无好友才给下一步动作
  if (!friendsLoading && !friendsError && friends.length === 0) {
    // F09：空好友簿不给死表单，给下一步动作
    return (
      <EmptyState
        icon={UserRoundPlus}
        title={t("uxk.group.emptyFriendsTitle")}
        description={t("uxk.group.emptyFriendsHint")}
        action={
          <Button type="button" size="sm" onClick={goAddFriend} data-testid="group-create-add-friend">
            {t("uxk.group.addFriendCta")}
          </Button>
        }
      />
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <Label htmlFor="group-create-name">{t("group.create.nameLabel")}</Label>
        <Input
          id="group-create-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={t("group.create.namePlaceholder")}
          data-testid="group-create-name"
          autoComplete="off"
        />
        {nameTooLong ? (
          <p className="text-destructive text-xs" role="alert" data-testid="group-create-name-too-long">
            {t("uxk.group.nameTooLong")}
          </p>
        ) : null}
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="group-create-friends-search">{t("group.create.membersLabel")}</Label>
        <EntityMultiSelect
          id="group-create-friends-search"
          options={memberOptions}
          selected={selected}
          onChange={setSelected}
          loading={friendsLoading}
          error={friendsError}
          onRetry={() => void ensureFriends()}
          warning={
            overCap
              ? t("group.manage.inviteOverCap", {
                  count: 1 + selected.length,
                  max: MAX_GROUP_MEMBERS,
                })
              : null
          }
          warningTestId="group-create-overcap"
          testId="group-create-friends"
        />
        {selected.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t("group.create.memberRequired")}</p>
        ) : null}
      </div>
      {commandError ? (
<CommandErrorText
          message={commandError}
          prefix={t("group.create.failed")}
          testId="group-create-error"
        />
      ) : null}
      <Button type="button" onClick={() => void submit()} disabled={!canSubmit} data-testid="group-create-submit">
        {submitting ? t("group.create.submitting") : t("group.create.submit")}
      </Button>
    </div>
  );
}
