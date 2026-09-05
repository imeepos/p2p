import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2Icon } from "lucide-react";

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
import { EMPTY_DRAFT, newEndpointId } from "@/acp/endpoint-storage";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import type { I18nKey } from "@/i18n/types";

import {
  hasEndpointFormErrors,
  validateEndpointForm,
  wsUrlHistory,
  type EndpointFormErrors,
} from "./endpoint-rules";
import { useEndpointTest } from "./use-endpoint-test";

function FieldError({ code }: { code?: string }) {
  const { t } = useTranslation();
  if (!code) return null;
  return (
    <p className="text-destructive text-xs" data-testid={"contacts-endpoint-error-" + code}>
      {t(("contacts.endpoint.errors." + code) as I18nKey)}
    </p>
  );
}

interface EndpointAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (endpoint: AcpEndpoint) => void;
}

// 添加 agent endpoint（§3.2/§3.4）：wsUrl 默认值沿用；wsUrl 历史值下拉
// （已保存端点去重、最近在前）；「测试连接」先行，未通过仍可保存（行内
// 警告徽标）；保存走 localStorage 收藏语义，endpointId 本地生成兜底。
export function EndpointAddDialog({ open, onOpenChange, onSaved }: EndpointAddDialogProps) {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const draft = useAcpStore((s) => s.draft);
  const upsertSaved = useAcpStore((s) => s.upsertSaved);
  const { start, testing } = useEndpointTest();
  const [form, setForm] = useState<AcpEndpoint>(draft);
  const [fieldErrors, setFieldErrors] = useState<EndpointFormErrors | null>(null);
  const idRef = useRef<string | null>(null);

  // 打开瞬间播种一次（渲染期状态调整，不落 effect）：表单回最近草稿
  const [seededOpen, setSeededOpen] = useState(false);
  if (open !== seededOpen) {
    setSeededOpen(open);
    if (open) {
      setForm(draft);
      setFieldErrors(null);
    }
  }

  const patch = (field: keyof AcpEndpoint) => (value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setFieldErrors(null);
  };

  const ensureId = (): string => {
    if (!idRef.current) idRef.current = form.endpointId?.trim() || newEndpointId();
    return idRef.current;
  };

  const validate = (): boolean => {
    const errors = validateEndpointForm({
      wsUrl: form.wsUrl,
      peer: form.peer,
      alias: form.alias ?? "",
    });
    if (hasEndpointFormErrors(errors)) {
      setFieldErrors(errors);
      return false;
    }
    setFieldErrors(null);
    return true;
  };

  const test = () => {
    if (!validate()) return;
    start({ ...form, endpointId: ensureId() });
  };

  const save = () => {
    if (!validate()) return;
    const stamped = upsertSaved({ ...form, endpointId: ensureId() });
    onOpenChange(false);
    onSaved(stamped);
  };

  const history = wsUrlHistory(saved);
  const activeId = idRef.current;
  const outcome = useEndpointMetaStore((s) =>
    activeId ? s.lastTest[activeId] ?? null : null,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="contacts-endpoint-dialog">
        <DialogHeader>
          <DialogTitle>{t("contacts.endpoint.title")}</DialogTitle>
          <DialogDescription>{t("contacts.endpoint.description")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-wsurl">{t("contacts.endpoint.wsUrlLabel")}</Label>
            <div className="flex gap-2">
              <Input
                id="contacts-endpoint-wsurl"
                className="font-mono text-xs"
                value={form.wsUrl}
                onChange={(e) => patch("wsUrl")(e.target.value)}
                placeholder={EMPTY_DRAFT.wsUrl}
                autoComplete="off"
                data-testid="contacts-endpoint-wsurl"
              />
              {history.length > 0 ? (
                <Select value="" onValueChange={(v) => v && patch("wsUrl")(v)} disabled={testing}>
                  <SelectTrigger className="w-28" data-testid="contacts-endpoint-history">
                    <SelectValue placeholder={t("contacts.endpoint.historyLabel")} />
                  </SelectTrigger>
                  <SelectContent>
                    {history.map((url) => (
                      <SelectItem key={url} value={url} data-testid={"contacts-endpoint-history-" + url}>
                        {url}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              ) : null}
            </div>
            <FieldError code={fieldErrors?.wsUrl} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-token">{t("contacts.endpoint.tokenLabel")}</Label>
            <Input
              id="contacts-endpoint-token"
              type="password"
              value={form.token}
              onChange={(e) => patch("token")(e.target.value)}
              autoComplete="off"
              data-testid="contacts-endpoint-token"
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-peer">{t("contacts.endpoint.peerLabel")}</Label>
            <Input
              id="contacts-endpoint-peer"
              className="font-mono text-xs"
              value={form.peer}
              onChange={(e) => patch("peer")(e.target.value)}
              autoComplete="off"
              data-testid="contacts-endpoint-peer"
            />
            <FieldError code={fieldErrors?.peer} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-alias">{t("contacts.endpoint.aliasLabel")}</Label>
            <Input
              id="contacts-endpoint-alias"
              value={form.alias ?? ""}
              onChange={(e) => patch("alias")(e.target.value)}
              autoComplete="off"
              data-testid="contacts-endpoint-alias"
            />
            <FieldError code={fieldErrors?.alias} />
          </div>
          <div className="flex items-center gap-2">
            <Button type="button" variant="outline" size="sm" onClick={test} disabled={testing} data-testid="contacts-endpoint-test">
              {testing ? (
                <>
                  <Loader2Icon aria-hidden className="size-4 animate-spin" />
                  {t("contacts.endpoint.testing")}
                </>
              ) : (
                t("contacts.endpoint.test")
              )}
            </Button>
            {outcome === "ok" ? (
              <span className="text-success text-xs" data-testid="contacts-endpoint-test-ok">
                {t("contacts.endpoint.testPassed")}
              </span>
            ) : null}
            {outcome === "failed" ? (
              <span className="text-destructive text-xs" data-testid="contacts-endpoint-test-failed">
                {t("contacts.endpoint.testFailed")}
              </span>
            ) : null}
          </div>
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
          <Button type="button" onClick={save} disabled={testing} data-testid="contacts-endpoint-save">
            {t("contacts.endpoint.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
