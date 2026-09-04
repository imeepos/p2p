import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { GroupJson } from "@/lib/ipc-types";
import { ipc } from "@/lib/ipc";
import { MAX_GROUP_MEMBERS } from "@/lib/mock-chat-rules";
import { useGroupStore } from "@/stores/group-store";

import { groupDisplayName } from "./group-names";

interface GroupCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 建群流程（设计 §5）：好友簿勾选成员 → 命名 → groupCreate → 群会话出现。
// 前置校验与 mock/后端同口径：群名非空、至少一名好友、上限 32（含本机）。
export function GroupCreateDialog({ open, onOpenChange }: GroupCreateDialogProps) {
  const { t } = useTranslation();
  const friends = useGroupStore((s) => s.friends);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const upsertGroup = useGroupStore((s) => s.upsertGroup);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) void ensureFriends();
  }, [open, ensureFriends]);

  const nameReady = name.trim().length > 0;
  const overCap = 1 + selected.size > MAX_GROUP_MEMBERS;
  const canSubmit = nameReady && selected.size > 0 && !overCap && !submitting;

  const toggle = (peerId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(peerId)) next.delete(peerId);
      else next.add(peerId);
      return next;
    });
  };

  const reset = () => {
    setName("");
    setSelected(new Set());
    setCommandError(null);
  };

  const submit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setCommandError(null);
    try {
      const group: GroupJson = await ipc.groupCreate(name.trim(), [...selected]);
      upsertGroup(group);
      try {
        await selectGroup(group.groupId);
      } catch (error) {
        // 已建群入列；仅首屏历史加载失败，不回滚建群，留日志信号。
        console.error("[group] 新群历史加载失败", error);
      }
      reset();
      onOpenChange(false);
    } catch (error) {
      console.error("[group] 建群失败", error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent className="sm:max-w-lg" data-testid="group-create-dialog">
        <DialogHeader>
          <DialogTitle>{t("group.create.title")}</DialogTitle>
          <DialogDescription>{t("group.description")}</DialogDescription>
        </DialogHeader>
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
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void submit()}
            disabled={!canSubmit}
            data-testid="group-create-submit"
          >
            {submitting ? t("group.create.submitting") : t("group.create.submit")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
