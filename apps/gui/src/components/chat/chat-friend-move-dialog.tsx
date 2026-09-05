import { useState } from "react";
import { useTranslation } from "react-i18next";

import { validateGroupName } from "@/lib/chat-limits";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { existingGroupNames } from "@/components/chat/chat-friend-group";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";

interface ChatFriendMoveDialogProps {
  friend: ChatFriendJson | null;
  onOpenChange: (open: boolean) => void;
}

// Radix SelectItem 禁用空串 value；未分组哨兵与折叠记忆键同款。
const UNGROUPED = "__ungrouped__";

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

// 移动到分组（IM-T43）：下拉选现有组点选即移动；输入新组名提交即创建并移入。
// IPC 调用在本组件（界面入口层，调用点守卫要求），成功后经 store 本地收尾。
export function ChatFriendMoveDialog({
  friend,
  onOpenChange,
}: ChatFriendMoveDialogProps) {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const updateFriendGroup = useChatStore((s) => s.updateFriendGroup);
  const [newGroup, setNewGroup] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

  const open = friend !== null;
  const groupNames = existingGroupNames(friends);
  const currentGroup = friend?.group ?? UNGROUPED;

  const reset = () => {
    setNewGroup(null);
    setFormError(null);
    setCommandError(null);
  };

  const move = async (group: string) => {
    if (!friend || moving) return;
    setCommandError(null);
    setMoving(true);
    try {
      // 空串 = 移出分组（镜像契约 §12.1：group 空串归一化未分组，不落盘空串）
      const updated = await ipc.chatFriendUpdate(friend.peerId, { group });
      updateFriendGroup(friend.peerId, updated.group ?? null);
      reset();
      onOpenChange(false);
    } catch (error) {
      console.error("[chat] 移动分组失败", friend.peerId, error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setMoving(false);
    }
  };

  // 下拉：现有组点选即移动；未分组 = 移出（后端收到空串）。
  const pickExisting = (value: string) => {
    void move(value === UNGROUPED ? "" : value);
  };

  // 手动输入：新分组名，提交即创建并移入（组已存在则等价普通移动）。
  const submitNew = async () => {
    if (!friend || moving) return;
    const trimmed = (newGroup ?? "").trim();
    if (trimmed.length === 0) return;
    const invalid = validateGroupName(trimmed);
    if (invalid) {
      setFormError(invalid);
      return;
    }
    setFormError(null);
    await move(trimmed);
  };

  const canSubmitNew = (newGroup ?? "").trim().length > 0;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!moving) {
          if (!next) reset();
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
        </DialogHeader>
        <div className="flex flex-col gap-1.5">
          <label className="text-sm" htmlFor="friend-move-select">
            {t("chat.group.selectLabel")}
          </label>
          <Select
            value={currentGroup}
            onValueChange={pickExisting}
            disabled={moving}
          >
            <SelectTrigger
              id="friend-move-select"
              className="w-full"
              data-testid="friend-move-select"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {groupNames.map((name) => (
                <SelectItem
                  key={name}
                  value={name}
                  data-testid={`friend-move-option-${name}`}
                >
                  {name}
                </SelectItem>
              ))}
              {groupNames.length > 0 && <SelectSeparator />}
              <SelectItem
                value={UNGROUPED}
                data-testid="friend-move-option-__ungrouped__"
              >
                {t("chat.group.ungrouped")}
              </SelectItem>
            </SelectContent>
          </Select>
          <DialogDescription>{t("chat.group.selectHint")}</DialogDescription>
          <FormError message={formError} />
          <CommandError message={commandError} />
        </div>
        <div className="flex flex-col gap-1.5">
          <label className="text-sm" htmlFor="friend-move-input">
            {t("chat.group.newLabel")}
          </label>
          <div className="flex gap-2">
            <input
              id="friend-move-input"
              value={newGroup ?? ""}
              onChange={(event) => {
                setNewGroup(event.target.value);
                setFormError(null);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && canSubmitNew) {
                  event.preventDefault();
                  void submitNew();
                }
              }}
              placeholder={t("chat.group.newPlaceholder")}
              className="w-full rounded-md border bg-transparent px-3 py-2 text-sm"
              data-testid="friend-move-input"
              maxLength={128}
              disabled={moving}
            />
            <Button
              type="button"
              onClick={() => void submitNew()}
              disabled={moving || !canSubmitNew}
              data-testid="friend-move-submit"
            >
              {moving ? t("chat.group.confirming") : t("chat.group.createSubmit")}
            </Button>
          </div>
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              reset();
              onOpenChange(false);
            }}
            disabled={moving}
            data-testid="friend-move-cancel"
          >
            {t("common.actions.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
