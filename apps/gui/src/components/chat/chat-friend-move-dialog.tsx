import { useState } from "react";
import { useTranslation } from "react-i18next";

import { validateGroupName } from "@/lib/mock-chat-rules";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { existingGroupNames } from "@/components/chat/chat-friend-group";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";

interface ChatFriendMoveDialogProps {
  friend: ChatFriendJson | null;
  onOpenChange: (open: boolean) => void;
}

// 表单预校验失败（i18n 文案）；后端拒绝走 CommandError 原文展示。
function FormError({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <p className="text-destructive text-xs" role="alert" data-testid="friend-move-invalid">
      {message}
    </p>
  );
}

// 后端拒绝：错误原文（Rust/mock 可读 Err）原样展示在框内，不翻译不吞。
function CommandError({ message }: { message: string | null }) {
  const { t } = useTranslation();
  if (!message) return null;
  return (
    <p className="text-destructive text-xs" role="alert" data-testid="friend-move-error">
      {t("chat.group.failed")}
      {message}
    </p>
  );
}

// 移动到分组（IM-T43）：输入或从现有组选（datalist）；清空提交 = 移出分组。
// IPC 调用在本组件（界面入口层，调用点守卫要求），成功后经 store 本地收尾。
export function ChatFriendMoveDialog({
  friend,
  onOpenChange,
}: ChatFriendMoveDialogProps) {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const updateFriendGroup = useChatStore((s) => s.updateFriendGroup);
  const [value, setValue] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

  const open = friend !== null;
  // value=null（未输入态）回显当前组，避免重开残留上一位好友的输入
  const current = value ?? (friend ? (friend.group ?? "") : "");
  const groupNames = existingGroupNames(friends);

  const submit = async () => {
    if (!friend || submitting) return;
    const trimmed = current.trim();
    const invalid = validateGroupName(trimmed);
    if (invalid) {
      setFormError(invalid);
      return;
    }
    setFormError(null);
    setCommandError(null);
    setSubmitting(true);
    try {
      // 空串 = 移出分组（镜像契约 §12.1：group 空串归一化未分组，不落盘空串）
      const updated = await ipc.chatFriendUpdate(friend.peerId, {
        group: trimmed,
      });
      updateFriendGroup(friend.peerId, updated.group ?? null);
      onOpenChange(false);
    } catch (error) {
      console.error("[chat] 移动分组失败", friend.peerId, error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!submitting) {
          if (!next) setValue(null);
          onOpenChange(next);
        }
      }}
    >
      <DialogContent
        className="sm:max-w-md"
        data-testid="friend-move-dialog"
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>{t("chat.group.moveTitle")}</DialogTitle>
          <DialogDescription>
            {friend?.nickname || friend?.peerId.slice(0, 12)}
          </DialogDescription>
          <DialogDescription>{t("chat.group.clearHint")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-1.5">
          <label className="text-sm" htmlFor="friend-move-input">
            {t("chat.group.nameLabel")}
          </label>
          <input
            id="friend-move-input"
            list="friend-group-options"
            value={current}
            onChange={(event) => {
              setValue(event.target.value);
              setFormError(null);
            }}
            placeholder={t("chat.group.namePlaceholder")}
            className="w-full rounded-md border bg-transparent px-3 py-2 text-sm"
            data-testid="friend-move-input"
            maxLength={128}
          />
          <datalist id="friend-group-options">
            {groupNames.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
          <FormError message={formError} />
          <CommandError message={commandError} />
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              setValue(null);
              onOpenChange(false);
            }}
            data-testid="friend-move-cancel"
          >
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void submit()}
            disabled={submitting}
            data-testid="friend-move-submit"
          >
            {submitting ? t("chat.group.confirming") : t("chat.group.submit")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
