import { useState } from "react";
import { useTranslation } from "react-i18next";
import { PlusIcon, Trash2Icon } from "lucide-react";

import { EntityCombobox, shortPeerId, type PickerOption } from "@/components/picker";
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
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
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
  /** 跨卡 URL 契约：#/contacts?add=<peerId> 挂载预填（自由文本兜底仍在） */
  initialPeerId?: string;
}

function FieldError({ code, errorId }: { code?: FriendFieldError; errorId: string }) {
  const { t } = useTranslation();
  if (!code) return null;
  return (
    <p className="text-destructive text-xs" role="alert" id={errorId}>
      {t(`chat.addFriend.${code}`)}
    </p>
  );
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

// F03：发现清单/节点表 → 选择器候选；在册好友显示昵称，其余用缩略 PeerId。
export function friendPickOptions(
  peers: Array<{ peerId: string }>,
  friends: Array<{ peerId: string; nickname: string }>,
): PickerOption[] {
  return peers.map((peer) => {
    const friend = friends.find((f) => f.peerId === peer.peerId);
    return {
      value: peer.peerId,
      label: friend?.nickname || shortPeerId(peer.peerId),
      hint: shortPeerId(peer.peerId),
    };
  });
}

// 行内校验错误与字段 aria 关联（F24）：invalid + describedby 指向错误节点，
// 错误节点 role=alert 保证读屏播报。
function peerAria(errorId: string, code?: FriendFieldError) {
  return {
    "aria-invalid": code ? true : undefined,
    "aria-describedby": code ? errorId : undefined,
  };
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
              {...peerAria(peerErrorId, fieldErrors?.peerId)}
            />
            <FieldError code={fieldErrors?.peerId} errorId={peerErrorId} />
          </div>
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

function NicknameField({
  value,
  onChange,
  error,
}: {
  value: string;
  onChange: (value: string) => void;
  error?: FriendFieldError;
}) {
  const { t } = useTranslation();
  const errorId = "friend-add-nickname-error";
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor="friend-add-nickname">{t("chat.addFriend.nicknameLabel")}</Label>
      <Input
        id="friend-add-nickname"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("chat.addFriend.nicknamePlaceholder")}
        autoComplete="off"
        {...peerAria(errorId, error)}
      />
      <FieldError code={error} errorId={errorId} />
    </div>
  );
}

function AddrRows({
  addrs,
  setAddrs,
  errors,
}: {
  addrs: string[];
  setAddrs: (update: (rows: string[]) => string[]) => void;
  errors: Record<number, FriendFieldError>;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-2">
      <Label>{t("chat.addFriend.addrsLabel")}</Label>
      {addrs.map((addr, index) => {
        const errorId = `friend-add-addr-error-${index}`;
        return (
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
                {...peerAria(errorId, errors[index])}
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
            <FieldError code={errors[index]} errorId={errorId} />
          </div>
        );
      })}
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
  );
}
