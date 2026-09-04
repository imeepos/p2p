import { useState } from "react";
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
import type { ChatFriendJson } from "@/lib/ipc-types";
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";

interface ChatFriendRemoveDialogProps {
  friend: ChatFriendJson | null;
  onOpenChange: (open: boolean) => void;
}

// 后端拒绝：错误原文（Rust/mock 可读 Err）原样展示在框内，不翻译不吞。
function CommandError({ message }: { message: string | null }) {
  const { t } = useTranslation();
  if (!message) return null;
  return (
    <p className="text-destructive text-xs" role="alert" data-testid="friend-remove-error">
      {t("chat.removeFriend.failed")}
      {message}
    </p>
  );
}

// 移除好友二次确认：文案如实说明移除不删本地消息历史；默认焦点在取消；
// 确认走 store.removeFriend，失败留在框内展示原文，好友列表保持原状。
export function ChatFriendRemoveDialog({
  friend,
  onOpenChange,
}: ChatFriendRemoveDialogProps) {
  const { t } = useTranslation();
  const forgetFriend = useChatStore((s) => s.forgetFriend);
  const [submitting, setSubmitting] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);

  const open = friend !== null;

  const confirm = async () => {
    if (!friend || submitting) return;
    setCommandError(null);
    setSubmitting(true);
    try {
      await ipc.chatFriendRemove(friend.peerId);
      markLocalWrite("chat");
      forgetFriend(friend.peerId);
      onOpenChange(false);
    } catch (error) {
      console.error("[chat] 移除好友失败", friend.peerId, error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  const displayName = friend ? friend.nickname || friend.peerId.slice(0, 12) : "";

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!submitting) onOpenChange(next);
      }}
    >
      <DialogContent
        className="sm:max-w-md"
        data-testid="friend-remove-dialog"
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>{t("chat.removeFriend.title")}</DialogTitle>
          <DialogDescription>
            {t("chat.removeFriend.description", { name: displayName })}
          </DialogDescription>
          <DialogDescription>{t("chat.removeFriend.historyNote")}</DialogDescription>
        </DialogHeader>
        <CommandError message={commandError} />
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            autoFocus
            onClick={() => onOpenChange(false)}
            data-testid="friend-remove-cancel"
          >
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={() => void confirm()}
            disabled={submitting}
            data-testid="friend-remove-confirm"
          >
            {submitting ? t("chat.removeFriend.confirming") : t("chat.removeFriend.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
