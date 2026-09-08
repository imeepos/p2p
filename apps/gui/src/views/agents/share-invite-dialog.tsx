// 分享邀请对话框（design §8.2）：owner 生成签名凭证邀请帧，
// 复制到剪贴板 + 显示一次性链接。v1 仅 copy，参考 llm-share redeem 对齐。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check } from "lucide-react";

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
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";
import { createInvite } from "@/a2a/admin-client";

import type { AgentDefJson } from "@/a2a/types";

interface ShareInviteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  agent: AgentDefJson | null;
}

export function ShareInviteDialog({ open, onOpenChange, agent }: ShareInviteDialogProps) {
  const { t } = useTranslation();
  const [inviteePeer, setInviteePeer] = useState("");
  const [inviteToken, setInviteToken] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleGenerate = async () => {
    if (!agent || !inviteePeer.trim()) return;
    setLoading(true);
    try {
      const descriptor = await ipc.acpLocalDescriptor();
      if (!descriptor) {
        toastError(t("contacts.agents.share.noDescriptor"));
        return;
      }
      const invite = await createInvite(
        descriptor.adminUrl,
        descriptor.token,
        agent.agentId,
        inviteePeer.trim(),
      );
      setInviteToken(JSON.stringify(invite));
      toastSuccess(t("contacts.agents.share.generated"));
    } catch (error) {
      toastError(`${t("contacts.agents.share.error")}: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = async () => {
    if (!inviteToken) return;
    try {
      await navigator.clipboard.writeText(inviteToken);
      setCopied(true);
      toastSuccess(t("contacts.agents.share.copied"));
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toastError(t("contacts.agents.share.copyError"));
    }
  };

  const handleClose = () => {
    setInviteePeer("");
    setInviteToken(null);
    setCopied(false);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-md" data-testid="share-invite-dialog">
        <DialogHeader>
          <DialogTitle>{t("contacts.agents.share.title")}</DialogTitle>
          <DialogDescription>
            {t("contacts.agents.share.description", { agentName: agent?.name ?? "" })}
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="invitee-peer">{t("contacts.agents.share.inviteePeer")}</Label>
            <Input
              id="invitee-peer"
              placeholder={t("contacts.agents.share.inviteePeerPlaceholder")}
              value={inviteePeer}
              onChange={(e) => setInviteePeer(e.target.value)}
              disabled={!!inviteToken}
              data-testid="share-invitee-input"
            />
          </div>
          {inviteToken && (
            <div className="flex flex-col gap-2">
              <Label>{t("contacts.agents.share.token")}</Label>
              <div className="flex items-center gap-2">
                <Input
                  readOnly
                  value={inviteToken}
                  className="font-mono text-xs"
                  data-testid="share-token-input"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleCopy}
                  data-testid="share-copy-btn"
                >
                  {copied ? (
                    <Check aria-hidden className="size-4" />
                  ) : (
                    <Copy aria-hidden className="size-4" />
                  )}
                </Button>
              </div>
            </div>
          )}
        </div>
        <DialogFooter>
          {!inviteToken ? (
            <Button
              type="button"
              onClick={handleGenerate}
              disabled={!inviteePeer.trim() || loading}
              data-testid="share-generate-btn"
            >
              {loading ? t("contacts.agents.share.generating") : t("contacts.agents.share.generate")}
            </Button>
          ) : (
            <Button type="button" onClick={handleClose} data-testid="share-done-btn">
              {t("common.actions.close")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}