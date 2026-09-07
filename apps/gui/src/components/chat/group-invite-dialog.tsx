import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { errorText } from "@/views/shared/form-flow";
import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";
import { usePeerNameLabel } from "@/lib/peer-name";
import { useChatStore } from "@/stores/chat-store";

// 入群确认弹框（IMC3 需求 1）：群信息（群名/群主/成员数口径说明）+ 邀请人 +
// 备注 + 同意/拒绝双按钮（同意为主按钮）。失败原文上浮不静默；同意成功本
// 组件直接跳 /chat?group=（roster 未达时由会话页加载态兜底）。

export interface GroupInviteTarget {
  message: ChatMessageJson;
  invite: GroupInviteJson | null;
}

interface GroupInviteDialogProps {
  target: GroupInviteTarget | null;
  onOpenChange: (open: boolean) => void;
}

function InfoRow({ label, value, testId }: { label: string; value: string; testId?: string }) {
  return (
    <div className="flex items-start justify-between gap-3 text-sm">
      <span className="text-muted-foreground shrink-0">{label}</span>
      <span className="text-right break-all" data-testid={testId}>{value}</span>
    </div>
  );
}

export function GroupInviteDialog({ target, onOpenChange }: GroupInviteDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const peerLabel = usePeerNameLabel();
  const acceptGroupInvite = useChatStore((s) => s.acceptGroupInvite);
  const rejectGroupInvite = useChatStore((s) => s.rejectGroupInvite);
  const [error, setError] = useState<string | null>(null);
  const body = target?.message.groupInvite ?? null;
  const invite = target?.invite ?? null;
  const open = target !== null;

  const close = () => onOpenChange(false);
  const fail = (err: unknown) => {
    console.error("[chat] 入群邀请操作失败", err);
    setError(errorText(err));
  };

  const accept = async () => {
    setError(null);
    if (!invite) {
      setError(t("chat.groupInvite.dialog.inviteMissing"));
      return;
    }
    try {
      await acceptGroupInvite(invite.id);
      toastSuccess(t("chat.groupInvite.dialog.acceptSucceeded"));
      close();
      // 群未达由会话页 GroupPendingPanel 兜底（roster 到达自动进入）
      navigate("/chat?group=" + (body?.groupId ?? invite.groupId));
    } catch (err) {
      fail(err);
    }
  };

  const reject = async () => {
    setError(null);
    if (!invite) {
      setError(t("chat.groupInvite.dialog.inviteMissing"));
      return;
    }
    try {
      await rejectGroupInvite(invite.id, null);
      toastSuccess(t("chat.groupInvite.dialog.rejectSucceeded"));
      close();
    } catch (err) {
      fail(err);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) setError(null); onOpenChange(next); }}>
      {open ? (
        <DialogContent className="sm:max-w-md" data-testid="group-invite-dialog">
          <DialogTitle>{t("chat.groupInvite.dialog.title")}</DialogTitle>
          <DialogDescription>
            {body?.groupName ?? ""}
          </DialogDescription>
          <div className="flex flex-col gap-2">
            <InfoRow
              label={t("chat.groupInvite.dialog.group")}
              value={body?.groupName ?? "-"}
              testId="group-invite-dialog-group"
            />
            <InfoRow
              label={t("chat.groupInvite.dialog.owner")}
              value={invite?.owner ? peerLabel(invite.owner) : "-"}
              testId="group-invite-dialog-owner"
            />
            <p className="text-muted-foreground text-xs">
              {t("chat.groupInvite.dialog.memberCountNote")}
            </p>
            <InfoRow
              label={t("chat.groupInvite.dialog.inviter")}
              value={body?.inviterNickname ?? "-"}
              testId="group-invite-dialog-inviter"
            />
            <InfoRow
              label={t("chat.groupInvite.dialog.note")}
              value={body?.note ?? t("chat.groupInvite.dialog.noNote")}
              testId="group-invite-dialog-note"
            />
            {error ? (
              <p className="text-destructive text-xs" role="alert" data-testid="group-invite-dialog-error">
                {error}
              </p>
            ) : null}
          </div>
          <div className="mt-2 flex items-center justify-end gap-2">
            <Button type="button" variant="outline" size="sm" onClick={reject}
              data-testid="group-invite-reject">
              {t("chat.groupInvite.dialog.reject")}
            </Button>
            <AsyncButton type="button" size="sm" action={accept}
              data-testid="group-invite-accept">
              {t("chat.groupInvite.dialog.accept")}
            </AsyncButton>
          </div>
        </DialogContent>
      ) : null}
    </Dialog>
  );
}
