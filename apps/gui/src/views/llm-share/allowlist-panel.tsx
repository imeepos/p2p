import { ShieldOff } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { CommandErrorText } from "@/components/feedback/command-error";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatDateTime } from "@/lib/format";
import type { Locale } from "@/i18n";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";
import { PeerNameCell } from "@/views/shared/peer-name-cell";
import { isValidFriendPeerId } from "@/views/contacts/chat-friend-rules";

import { focusFirstInvalidField } from "./focus-first-error";
import { PeerIdField } from "./peer-id-field";

import type { LlmAllowEntry, LlmShareBackend } from "./types";

interface AllowFormValues {
  peerId: string;
  models: string;
  note: string;
}

const EMPTY_ALLOW_FORM: AllowFormValues = { peerId: "", models: "", note: "" };

// R2-05：PeerId 即时校验（F14 时机：首次失焦前不打断输入），提交兜底拦截
function peerErrorKeyOf(value: string, touched: boolean): I18nKey | null {
  if (!touched) return null;
  const trimmed = value.trim();
  if (!trimmed) return "llmShare.allowlist.errPeerRequired";
  if (!isValidFriendPeerId(trimmed)) return "llmShare.allowlist.errPeerInvalid";
  return null;
}

function parseAllowModels(text: string): string[] {
  return text
    .split(/[,\n，、]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function AllowRow({ entry, onDeny, busy }: { entry: LlmAllowEntry; onDeny: (peerId: string) => void; busy: boolean }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const models =
    entry.models.length > 0 ? entry.models.join(", ") : t("llmShare.allowlist.unlimitedModels");
  return (
    <TableRow data-testid="allow-row">
      <TableCell className="max-w-44">
        <PeerNameCell peerId={entry.peerId} />
      </TableCell>
      <TableCell className="max-w-44 truncate text-xs" title={models}>{models}</TableCell>
      <TableCell className="max-w-44 truncate text-xs" title={entry.note ?? ""}>{entry.note}</TableCell>
      <TableCell className="text-xs">
        {formatDateTime(new Date(entry.grantedAt).getTime(), locale)}
      </TableCell>
      <TableCell>
        <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => onDeny(entry.peerId)}>
          {t("llmShare.allowlist.deny")}
        </Button>
      </TableCell>
    </TableRow>
  );
}

export function AllowlistPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [entries, setEntries] = useState<LlmAllowEntry[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [form, setForm] = useState<AllowFormValues>(EMPTY_ALLOW_FORM);
  const [busy, setBusy] = useState(false);
  const [peerTouched, setPeerTouched] = useState(false);
  const peerErrorKey = peerErrorKeyOf(form.peerId, peerTouched);

  const refresh = useCallback(async () => {
    try {
      const view = await backend.allowList();
      setEntries(view.entries);
      setLoadError(null);
    } catch (error) {
      console.warn("[llm-share] allowlist 读取失败", error);
      setLoadError(errorText(error));
      setEntries(null);
    }
  }, [backend]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const view = await backend.allowList();
        if (!cancelled) {
          setEntries(view.entries);
          setLoadError(null);
        }
      } catch (error) {
        console.warn("[llm-share] allowlist 读取失败", error);
        if (!cancelled) setLoadError(errorText(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  // deny 不存在条目是默认拒绝语义的显式报错（非故障）：原样露出不吞
  const runAction = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setActionError(null);
    try {
      await action();
      await refresh();
    } catch (error) {
      console.warn("[llm-share] allowlist 操作失败", error);
      setActionError(errorText(error));
    } finally {
      setBusy(false);
    }
  };

  const handleAllow = (event: FormEvent) => {
    event.preventDefault();
    setPeerTouched(true);
    // R2-12：与 aria-invalid 同源判定，聚焦错误字段（当前仅 PeerId 一项）
    const error = peerErrorKeyOf(form.peerId, true);
    focusFirstInvalidField(["llm-allow-peer"], () => error != null);
    if (error) return;
    void runAction(async () => {
      await backend.allow({
        peerId: form.peerId.trim(),
        // models 留空 = 不限模型（ai-guide allow 语义），后端决定缺省形状
        models: parseAllowModels(form.models),
        note: form.note.trim() || undefined,
      });
      setForm(EMPTY_ALLOW_FORM);
    });
  };

  // R2-02：移出即撤销借用授权（破坏性），走全站统一的二次确认，文案说明
  // 后果与恢复路径（再次加入即恢复）。
  const handleDeny = (peerId: string) => {
    void (async () => {
      const ok = await confirm({
        title: t("llmShare.allowlist.denyConfirmTitle"),
        description: t("llmShare.allowlist.denyConfirmDesc"),
        confirmText: t("llmShare.allowlist.deny"),
        cancelText: t("common.actions.cancel"),
        destructive: true,
      });
      if (!ok) return;
      await runAction(() => backend.deny(peerId));
    })();
  };

  const set = (field: keyof AllowFormValues) => (value: string) =>
    setForm((v) => ({ ...v, [field]: value }));

  return (
    <div className="flex flex-col gap-3" data-testid="allowlist-panel">
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">{t("llmShare.panels.allowlist")}</CardTitle>
        </CardHeader>
        <CardContent>
          <form className="flex flex-col gap-3" onSubmit={handleAllow} noValidate>
            <PeerIdField
              label={t("llmShare.allowlist.formPeerId")}
              inputId="llm-allow-peer"
              value={form.peerId}
              onValueChange={set("peerId")}
              onBlur={() => setPeerTouched(true)}
              errorKey={peerErrorKey}
              errorId="llm-allow-peer-error"
            />
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-allow-models">{t("llmShare.allowlist.formModels")}</Label>
                <Input
                  id="llm-allow-models"
                  value={form.models}
                  onChange={(e) => set("models")(e.target.value)}
                  placeholder={t("llmShare.allowlist.formModelsOptional")}
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-allow-note">{t("llmShare.allowlist.formNote")}</Label>
                <Input
                  id="llm-allow-note"
                  value={form.note}
                  onChange={(e) => set("note")(e.target.value)}
                />
              </div>
            </div>
            {actionError ? (
              <CommandErrorText
                message={actionError}
                prefix={t("llmShare.allowlist.actionFailed") + "："}
              />
            ) : null}
            <div>
              <Button type="submit" size="sm" disabled={busy}>
                {t("llmShare.allowlist.allow")}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
      {entries && entries.length > 0 ? (
        <Card>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("llmShare.allowlist.columnPeer")}</TableHead>
                  <TableHead>{t("llmShare.allowlist.columnModels")}</TableHead>
                  <TableHead>{t("llmShare.allowlist.columnNote")}</TableHead>
                  <TableHead>{t("llmShare.allowlist.columnGrantedAt")}</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow><TableCell colSpan={5} className="text-muted-foreground text-xs">{t("llmShare.allowlist.count", { count: entries.length })}</TableCell></TableRow>
                {entries.map((entry) => (
                  <AllowRow key={entry.peerId} entry={entry} onDeny={handleDeny} busy={busy} />
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        <EmptyState
          icon={ShieldOff}
          title={t("llmShare.allowlist.emptyTitle")}
          description={loadError ?? t("llmShare.allowlist.emptyHint")}
        />
      )}
    </div>
  );
}
