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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toastError } from "@/components/feedback/toast";
import { CopyButton } from "@/components/monitor/copy-button";
import { ShareCreateFields } from "./share-create-fields";
import { ShareSendTargets } from "./share-send-targets";
import { useAcpStore } from "@/acp/acp-store";
import { adminEndpointCandidates } from "@/acp/admin-endpoints";
import { useLocalAdminCandidate } from "@/acp/use-local-admin";
import { createShare, listWorkspaces, type AcpWorkspace } from "@/acp/share-admin-client";
import {
  hasShareCreateErrors,
  shareCreateBody,
  validateShareCreate,
  type ShareScope,
  type ShareTtlKey,
} from "@/acp/share-model";
import type { I18nKey } from "@/i18n/types";

interface ShareCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 聊天页入口传入：把链接作为普通文本消息发进当前会话（§8） */
  onSendLink?: (link: string) => Promise<void>;
  /** 本地工作区行入口预选的 workspace id。 */
  initialWorkspaceId?: string;
}

/** owner 创建分享弹层（§8）：scope/有效期档位/激活次数/备注 → admin POST /shares
 *  → 展示链接 + 复制 + 发送到当前聊天。admin 端点候选来自 endpoint 登记。 */
export function ShareCreateDialog({ open, onOpenChange, onSendLink, initialWorkspaceId }: ShareCreateDialogProps) {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const draft = useAcpStore((s) => s.draft);
  const candidates = adminEndpointCandidates(saved, draft);
  // 无已登记管理端点时自动发现本机 agent（自描述文件），免手填 token
  const { candidate: localCandidate, done: localDone } = useLocalAdminCandidate(
    open && candidates.length === 0,
    t("acp.share.localCandidateLabel"),
  );

  const [endpointId, setEndpointId] = useState<string | null>(null);
  const [scope, setScope] = useState<ShareScope>("sandbox");
  const [workspaces, setWorkspaces] = useState<AcpWorkspace[] | null>(null);
  const [workspaceId, setWorkspaceId] = useState<string>("");
  const [ttl, setTtl] = useState<ShareTtlKey>("1h");
  const [activations, setActivations] = useState("1");
  const [note, setNote] = useState("");
  const [errors, setErrors] = useState<ReturnType<typeof validateShareCreate> | null>(null);
  const [creating, setCreating] = useState(false);
  const [link, setLink] = useState<string | null>(null);
  const [sent, setSent] = useState(false);

  // 打开瞬间播种一次（渲染期状态调整，不落 effect）：重置表单回默认档
  const [seededOpen, setSeededOpen] = useState(false);
  if (open !== seededOpen) {
    setSeededOpen(open);
    if (open) {
      setEndpointId(null);
      setScope("sandbox");
      setWorkspaces(null);
      setWorkspaceId(initialWorkspaceId ?? "");
      setTtl("1h");
      setActivations("1");
      setNote("");
      setErrors(null);
      setLink(null);
      setSent(false);
    }
  }

  const endpoint = candidates.find((c) => c.id === endpointId) ?? candidates[0] ?? localCandidate;

  // 工作区清单（多工作区加法）：端点就绪且选中 workspace 范围后才拉取；
  // 旧 agent 无此端点 → 空表回落默认工作区（body 不带 workspace 字段）。
  const endpointUrl_ = endpoint?.url ?? null;
  const endpointToken_ = endpoint?.token ?? "";
  useEffect(() => {
    if (!open || scope !== "workspace" || !endpointUrl_) return;
    let dead = false;
    listWorkspaces(endpointUrl_, endpointToken_)
      .then((rows) => {
        if (dead) return;
        setWorkspaces(rows);
        // 展示的默认项即定向目标：未显式选择时播种首行（所见即所分享）
        setWorkspaceId((prev) => prev || rows[0]?.id || "");
      })
      .catch(() => {
        if (!dead) setWorkspaces([]);
      });
    return () => {
      dead = true;
    };
  }, [open, scope, endpointUrl_, endpointToken_]);

  const create = async () => {
    const maxActivations = Number(activations);
    const formErrors = validateShareCreate({ maxActivations, note });
    if (hasShareCreateErrors(formErrors)) {
      setErrors(formErrors);
      return;
    }
    if (!endpoint) return;
    setErrors(null);
    setCreating(true);
    try {
      const out = await createShare(endpoint.url, endpoint.token, shareCreateBody({
        scope,
        workspaceId: scope === "workspace" ? workspaceId || null : null,
        ttl,
        maxActivations,
        note,
      }));
      setLink(out.link);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const key: I18nKey = message.startsWith("HTTP 422")
        ? "acp.share.createRejectedWorkspace"
        : "acp.share.createFailed";
      toastError(t(key), { description: message, context: "acp-share-create" });
    } finally {
      setCreating(false);
    }
  };

  const sendToChat = async () => {
    if (!link || !onSendLink) return;
    try {
      await onSendLink(link);
      setSent(true);
      onOpenChange(false);
    } catch (error) {
      toastError(t("chat.sendFailed"), {
        description: error instanceof Error ? error.message : String(error),
        context: "acp-share-send",
      });
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="acp-share-dialog">
        <DialogHeader>
          <DialogTitle>{t("acp.share.dialogTitle")}</DialogTitle>
          <DialogDescription>{t("acp.share.dialogDescription")}</DialogDescription>
        </DialogHeader>
        {candidates.length === 0 && localCandidate ? (
          <p className="text-muted-foreground text-xs" data-testid="acp-share-local-auto">
            {t("acp.share.localAutoHint", { url: localCandidate.url })}
          </p>
        ) : null}
        {candidates.length === 0 && localDone && !localCandidate ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-share-admin-missing">
            {t("acp.share.adminMissing")}
          </p>
        ) : (
          <div className="flex flex-col gap-3">
            {candidates.length > 1 ? (
              <div className="flex flex-col gap-1">
                <Label>{t("acp.share.adminEndpoint")}</Label>
                <Select value={endpoint?.id ?? ""} onValueChange={setEndpointId}>
                  <SelectTrigger className="w-full" data-testid="acp-share-admin-select">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {candidates.map((c) => (
                      <SelectItem key={c.id} value={c.id}>
                        {c.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            ) : null}
            <ShareCreateFields
              scope={scope}
              onScopeChange={setScope}
              ttl={ttl}
              onTtlChange={setTtl}
              activations={activations}
              onActivationsChange={(v) => {
                setActivations(v);
                setErrors(null);
              }}
              note={note}
              onNoteChange={(v) => {
                setNote(v);
                setErrors(null);
              }}
              errors={errors}
              onClearErrors={() => setErrors(null)}
              workspaces={workspaces}
              workspaceId={workspaceId}
              onWorkspaceChange={setWorkspaceId}
            />

            {link ? (
              <>
                <div className="flex flex-col gap-1" data-testid="acp-share-link-area">
                  <Label>{t("acp.share.linkLabel")}</Label>
                  <div className="flex items-center gap-1">
                    <Input readOnly value={link} className="font-mono text-xs" data-testid="acp-share-link" />
                    <CopyButton value={link} data-testid="acp-share-copy" />
                  </div>
                </div>
                <ShareSendTargets link={link} />
              </>
            ) : null}
          </div>
        )}
        <DialogFooter>
          {link && onSendLink && !sent ? (
            <Button type="button" onClick={() => void sendToChat()} data-testid="acp-share-send">
              {t("acp.share.sendToChat")}
            </Button>
          ) : null}
          <Button
            type="button"
            variant={link ? "outline" : "default"}
            onClick={() => void create()}
            disabled={creating || (candidates.length === 0 && !localCandidate)}
            data-testid="acp-share-create"
          >
            {creating ? t("acp.share.creating") : t("acp.share.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}