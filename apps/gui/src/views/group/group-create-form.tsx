import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { UserRoundPlus } from "lucide-react";

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

// 建群表单（设计 §5）：好友簿勾选成员 → 命名 → groupCreate。F09 起被
// GroupCreateDialog 与联系人「添加群聊」弹窗共用，两处一跳直达。
// F10：文案说用户语言（1-64 个字），trim 内部规则交给校验不上文案。
// 前置校验与 mock/后端同口径：群名非空、至少一名好友、上限 32（含本机）。
export function GroupCreateForm({ onDone }: GroupCreateFormProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const friends = useGroupStore((s) => s.friends);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const upsertGroup = useGroupStore((s) => s.upsertGroup);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    void ensureFriends();
  }, [ensureFriends]);

  const trimmedName = name.trim();
  const nameTooLong = Array.from(trimmedName).length > MAX_GROUP_NAME_CHARS;
  const overCap = 1 + selected.size > MAX_GROUP_MEMBERS;
  const canSubmit =
    trimmedName.length > 0 && !nameTooLong && selected.size > 0 && !overCap && !submitting;

  const toggle = (peerId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(peerId)) next.delete(peerId);
      else next.add(peerId);
      return next;
    });
  };

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
      const group: GroupJson = await ipc.groupCreate(trimmedName, [...selected]);
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

  if (friends.length === 0) {
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
        <Label>{t("group.create.membersLabel")}</Label>
        <div
          data-testid="group-create-friends"
          className="scroll-slim max-h-56 overflow-y-auto rounded-md border p-1"
        >
          {friends.map((friend) => (
            <label
              key={friend.peerId}
              className="hover:bg-accent flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-sm"
            >
              <input
                type="checkbox"
                className="size-4"
                checked={selected.has(friend.peerId)}
                onChange={() => toggle(friend.peerId)}
                data-testid={`group-create-friend-${friend.peerId}`}
              />
              <span className="truncate">{groupDisplayName(friend.peerId, friends)}</span>
              <span className="text-muted-foreground ml-auto truncate text-xs">
                {friend.peerId.slice(0, 12)}
              </span>
            </label>
          ))}
        </div>
        {selected.size === 0 ? (
          <p className="text-xs text-muted-foreground">{t("group.create.memberRequired")}</p>
        ) : null}
        {overCap ? (
          <p className="text-destructive text-xs" data-testid="group-create-overcap">
            {t("group.manage.inviteOverCap", {
              count: 1 + selected.size,
              max: MAX_GROUP_MEMBERS,
            })}
          </p>
        ) : null}
      </div>
      {commandError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="group-create-error">
          {t("group.create.failed")}
          {commandError}
        </p>
      ) : null}
      <Button type="button" onClick={() => void submit()} disabled={!canSubmit} data-testid="group-create-submit">
        {submitting ? t("group.create.submitting") : t("group.create.submit")}
      </Button>
    </div>
  );
}
