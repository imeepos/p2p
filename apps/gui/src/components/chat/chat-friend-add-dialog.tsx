import { useState } from "react";
import { useTranslation } from "react-i18next";
import { PlusIcon, Trash2Icon } from "lucide-react";

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
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";

import {
  hasFriendFormErrors,
  validateFriendForm,
  type FriendFieldError,
  type FriendFormErrors,
} from "./chat-friend-rules";

interface ChatFriendAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function FieldError({ code }: { code?: FriendFieldError }) {
  const { t } = useTranslation();
  if (!code) return null;
  return <p className="text-destructive text-xs">{t(`chat.addFriend.${code}`)}</p>;
}

// 后端拒绝：错误原文（Rust/mock 可读 Err）原样展示在表单内，不翻译不吞。
function CommandError({ message }: { message: string | null }) {
  const { t } = useTranslation();
  if (!message) return null;
  return (
    <p className="text-destructive text-xs" role="alert" data-testid="friend-add-error">
      {t("chat.addFriend.failed")}
      {message}
    </p>
  );
}

// 添加好友表单：PeerId 必填，昵称/地址选填；前端预校验与后端同口径，
// 后端拒绝保留已填内容并把原文展示在表单内；成功后刷新列表并选中新好友。
export function ChatFriendAddDialog({ open, onOpenChange }: ChatFriendAddDialogProps) {
  const { t } = useTranslation();
  const loadFriends = useChatStore((s) => s.loadFriends);
  const selectPeer = useChatStore((s) => s.selectPeer);
  const [peerId, setPeerId] = useState("");
  const [nickname, setNickname] = useState("");
  const [addrs, setAddrs] = useState<string[]>([]);
  const [fieldErrors, setFieldErrors] = useState<FriendFormErrors | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const reset = () => {
    setPeerId("");
    setNickname("");
    setAddrs([]);
    setFieldErrors(null);
    setCommandError(null);
  };

  const handleOpenChange = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  const submit = async () => {
    const errors = validateFriendForm(peerId, nickname, addrs);
    if (hasFriendFormErrors(errors)) {
      setFieldErrors(errors);
      return;
    }
    setFieldErrors(null);
    setCommandError(null);
    setSubmitting(true);
    try {
      const friend = await ipc.chatFriendAdd(
        peerId.trim(),
        nickname.trim(),
        addrs.map((addr) => addr.trim()).filter((addr) => addr.length > 0),
      );
      await loadFriends();
      try {
        await selectPeer(friend.peerId);
      } catch (error) {
        // 好友已入簿；仅历史加载失败，不回滚添加，留日志信号。
        console.error("[chat] 新好友历史加载失败", error);
      }
      handleOpenChange(false);
    } catch (error) {
      console.error("[chat] 添加好友失败", error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="friend-add-dialog">
        <DialogHeader>
          <DialogTitle>{t("chat.addFriend.title")}</DialogTitle>
          <DialogDescription>{t("chat.addFriend.description")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-add-peer-id">{t("chat.addFriend.peerIdLabel")}</Label>
            <Input
              id="friend-add-peer-id"
              className="font-mono text-xs"
              value={peerId}
              onChange={(event) => setPeerId(event.target.value)}
              placeholder={t("chat.addFriend.peerIdPlaceholder")}
              autoComplete="off"
            />
            <FieldError code={fieldErrors?.peerId} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-add-nickname">{t("chat.addFriend.nicknameLabel")}</Label>
            <Input
              id="friend-add-nickname"
              value={nickname}
              onChange={(event) => setNickname(event.target.value)}
              placeholder={t("chat.addFriend.nicknamePlaceholder")}
              autoComplete="off"
            />
            <FieldError code={fieldErrors?.nickname} />
          </div>
          <div className="flex flex-col gap-2">
            <Label>{t("chat.addFriend.addrsLabel")}</Label>
            {addrs.map((addr, index) => (
              <div key={index} className="flex flex-col gap-1">
                <div className="flex items-center gap-2">
                  <Input
                    className="font-mono text-xs"
                    value={addr}
                    onChange={(event) =>
                      setAddrs((rows) =>
                        rows.map((row, i) => (i === index ? event.target.value : row)),
                      )
                    }
                    placeholder={t("chat.addFriend.addrPlaceholder")}
                    aria-label={`${t("chat.addFriend.addrsLabel")} ${index + 1}`}
                    autoComplete="off"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label={t("chat.addFriend.removeAddr")}
                    onClick={() => setAddrs((rows) => rows.filter((_, i) => i !== index))}
                  >
                    <Trash2Icon aria-hidden />
                  </Button>
                </div>
                <FieldError code={fieldErrors?.addrs[index]} />
              </div>
            ))}
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="w-fit"
              onClick={() => setAddrs((rows) => [...rows, ""])}
            >
              <PlusIcon aria-hidden />
              {t("chat.addFriend.addAddr")}
            </Button>
          </div>
          <CommandError message={commandError} />
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => handleOpenChange(false)}
          >
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void submit()}
            disabled={submitting}
            data-testid="friend-add-submit"
          >
            {submitting ? t("chat.addFriend.submitting") : t("chat.addFriend.submit")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
