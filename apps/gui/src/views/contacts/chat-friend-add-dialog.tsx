import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { EntityCombobox } from "@/components/picker";
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
import { toastSuccess } from "@/components/feedback/toast";
import { markLocalWrite } from "@/lib/data-watch";
import { isValidPeerId } from "@/lib/chat-limits";
import { ipc } from "@/lib/ipc";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { useChatStore } from "@/stores/chat-store";
import { usePeerProfileStore } from "@/stores/peer-profile-store";

import {
  fieldErrorAria,
  friendPickOptions,
  hasFriendFormErrors,
  validateFriendForm,
  type FriendFormErrors,
} from "./chat-friend-rules";
import { AddrRows, FieldError, NicknameField } from "./chat-friend-add-fields";
import { PeerProfilePreview } from "./peer-profile-preview";

interface ChatFriendAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 跨卡 URL 契约：#/contacts?add=<peerId> 挂载预填（自由文本兜底仍在） */
  initialPeerId?: string;
}

// 后端拒绝：错误原文（Rust/mock 可读 Err）原样展示在表单内，不翻译不吞。
function CommandError({ message }: { message: string | null }) {
  const { t } = useTranslation();
  return (
    <CommandErrorText
      message={message}
      prefix={t("chat.addFriend.failed")}
      testId="friend-add-error"
    />
  );
}

// 添加好友表单：PeerId 必填（选择器辅助填充 + 自由文本兜底），昵称/地址选填；
// 前端预校验与后端同口径，后端拒绝保留已填内容并把原文展示在表单内。
export function ChatFriendAddDialog({ open, onOpenChange, initialPeerId }: ChatFriendAddDialogProps) {
  const { t } = useTranslation();
  const loadFriends = useChatStore((s) => s.loadFriends);
  const loadInvites = useChatStore((s) => s.loadInvites);
  const friends = useChatStore((s) => s.friends);
  const discovered = useNodeStore(selectPeerList);
  // URL 契约预填：挂载即开（open 首渲染即 true）时初值直达，不能依赖
  // 开启瞬间的状态迁移播种。关闭重开由 reset() 归零，与既有行为一致。
  const [peerId, setPeerId] = useState(initialPeerId ?? "");
  const [nickname, setNickname] = useState("");
  const [addrs, setAddrs] = useState<string[]>([]);
  const [fieldErrors, setFieldErrors] = useState<FriendFormErrors | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const fetchProfile = usePeerProfileStore((s) => s.fetch);

  // 对端自报资料预取（契约 §12.1）：peerId 合法即拉取；昵称仅在留空时用
  // 对端名称预填（用户已输入不覆盖），弹窗内卡片实时展示头像/简介。
  useEffect(() => {
    if (!isValidPeerId(peerId)) return;
    let cancelled = false;
    void fetchProfile(peerId).then((profile) => {
      if (cancelled) return;
      if (profile?.name) {
        setNickname((prev) => (prev.trim() ? prev : profile.name));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [peerId, fetchProfile]);

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
      await ipc.chatFriendInvite(
        peerId.trim(),
        nickname.trim(),
        addrs.map((addr) => addr.trim()).filter((addr) => addr.length > 0),
      );
      markLocalWrite("chat");
      await loadFriends();
      await loadInvites();
      toastSuccess(t("contacts.friends.addSuccess"));
      handleOpenChange(false);
    } catch (error) {
      console.error("[chat] 添加好友失败", error);
      setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  const options = friendPickOptions(discovered, friends);
  const peerErrorId = "friend-add-peer-id-error";

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="friend-add-dialog">
        <DialogHeader>
          <DialogTitle>{t("chat.addFriend.title")}</DialogTitle>
          <DialogDescription>{t("chat.addFriend.description")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-add-peer-pick">{t("picker.friendPickLabel")}</Label>
            <EntityCombobox
              id="friend-add-peer-pick"
              options={options}
              value={options.some((option) => option.value === peerId) ? peerId : null}
              onChange={(value) => {
                if (value) setPeerId(value);
                else setPeerId("");
              }}
              testId="friend-add-picker"
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="friend-add-peer-id">{t("chat.addFriend.peerIdLabel")}</Label>
            <Input
              id="friend-add-peer-id"
              className="font-mono text-xs"
              value={peerId}
              onChange={(event) => setPeerId(event.target.value)}
              placeholder={t("chat.addFriend.peerIdPlaceholder")}
              autoComplete="off"
              {...fieldErrorAria(peerErrorId, fieldErrors?.peerId)}
            />
            <FieldError code={fieldErrors?.peerId} errorId={peerErrorId} />
          </div>
          <PeerProfilePreview peerId={peerId.trim()} />
          <NicknameField
            value={nickname}
            onChange={setNickname}
            error={fieldErrors?.nickname}
          />
          <AddrRows addrs={addrs} setAddrs={setAddrs} errors={fieldErrors?.addrs} />
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
