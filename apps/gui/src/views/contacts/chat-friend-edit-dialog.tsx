import { useState } from "react";
import { useTranslation } from "react-i18next";

import { CommandErrorText } from "@/components/feedback/command-error";
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
import { Textarea } from "@/components/ui/textarea";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { errorText } from "@/views/shared/form-flow";

interface ChatFriendEditDialogProps {
  friend: ChatFriendJson;
  onOpenChange: (open: boolean) => void;
}

// 好友资料编辑弹窗（IM-T43 消费面）：显示名 + 备注；空显示名提交 = 回退
// PeerId 缩略（crate friend_update 语义），空备注 = 清除备注。
// 挂载期初始化表单值（调用方条件渲染，关闭即卸载），无 effect 播种。
export function ChatFriendEditDialog({ friend, onOpenChange }: ChatFriendEditDialogProps) {
  const { t } = useTranslation();
  const updateFriend = useChatStore((s) => s.updateFriend);
  const [nickname, setNickname] = useState(friend.nickname);
  const [note, setNote] = useState(friend.note ?? "");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await updateFriend(friend.peerId, {
        nickname: nickname.trim(),
        note: note.trim(),
      });
      toastSuccess(t("contacts.friends.editSuccess"));
      onOpenChange(false);
    } catch (err) {
      console.error("[contacts] 好友资料保存失败", err);
      const detail = errorText(err);
      setError(detail);
      toastError(t("contacts.friends.editFailed"), { description: detail, context: "chat_friend_update" });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" data-testid="friend-edit-dialog">
        <DialogHeader>
          <DialogTitle>{t("contacts.friends.editTitle")}</DialogTitle>
          <DialogDescription>{t("contacts.friends.editDescription")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-edit-nickname">{t("contacts.friends.editNicknameLabel")}</Label>
            <Input
              id="friend-edit-nickname"
              value={nickname}
              onChange={(event) => setNickname(event.target.value)}
              placeholder={t("contacts.friends.editNicknamePlaceholder")}
              maxLength={64}
              autoComplete="off"
              data-testid="friend-edit-nickname"
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-edit-note">{t("contacts.detail.remark")}</Label>
            <Textarea
              id="friend-edit-note"
              value={note}
              onChange={(event) => setNote(event.target.value)}
              placeholder={t("contacts.friends.editNotePlaceholder")}
              rows={3}
              data-testid="friend-edit-note"
            />
          </div>
          {error ? (
            <CommandErrorText message={error} prefix={t("contacts.friends.editFailed")} testId="friend-edit-error" />
          ) : null}
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void submit()}
            disabled={submitting}
            data-testid="friend-edit-submit"
          >
            {t("common.actions.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
