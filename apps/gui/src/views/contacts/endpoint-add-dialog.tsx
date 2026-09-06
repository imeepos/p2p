import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { ChevronDown, Loader2Icon } from "lucide-react";

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
import { useDiscoveryPoll } from "@/acp/use-discovery-poll";

import { defaultAdminUrl, hasEndpointFormErrors, targetOptions, validateEndpointForm, wsUrlHistory } from "./endpoint-rules";
import { AdvancedFields, EndpointFieldError } from "./endpoint-advanced-fields";
import { useEndpointTest } from "./use-endpoint-test";

interface EndpointAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (endpoint: AcpEndpoint) => void;
}

/** 打开瞬间播种（渲染期状态调整，不落 effect）：console ready 时以本机控制台值
 *  预填连接面（未动过的字段才覆盖），补管理地址缺省，并预选首个发现目标 */
function seedForm(draft: AcpEndpoint, consoleStatus: ReturnType<typeof useAcpStore.getState>["console"]): AcpEndpoint {
  const seeded = { ...draft };
  const ready = consoleStatus?.phase === "ready" ? consoleStatus : null;
  if (ready) {
    if (!seeded.wsUrl.trim() || seeded.wsUrl === EMPTY_DRAFT.wsUrl) {
      seeded.wsUrl = ready.wsUrl ?? seeded.wsUrl;
    }
    if (!seeded.token.trim()) seeded.token = ready.token ?? seeded.token;
    if (!seeded.statusUrl?.trim()) seeded.statusUrl = ready.statusUrl;
    if (!seeded.adminUrl?.trim()) seeded.adminUrl = ready.adminUrl;
  }
  if (!seeded.adminUrl?.trim()) {
    const derived = defaultAdminUrl(seeded.wsUrl);
    if (derived) seeded.adminUrl = derived;
  }
  return seeded;
}

/** 添加 agent endpoint（§3.2/§3.4 + UX3 收敛）：主字段 = 目标节点从发现清单下拉
 *  选择（禁自由文本）；wsUrl/token/peer/分享管理收进「高级」折叠，默认以本机
 *  console 值预填；wsUrl 历史值下拉与「保存/测试连接」既有语义不退化。
 *  主按钮「添加并开始对话」= 保存 + 测试连接（连接即测试）+ 跳转会话一条龙。 */
export function EndpointAddDialog({ open, onOpenChange, onSaved }: EndpointAddDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const saved = useAcpStore((s) => s.saved);
  const directory = useAcpStore((s) => s.directory);
  const consoleStatus = useAcpStore((s) => s.console);
  const draft = useAcpStore((s) => s.draft);
  const upsertSaved = useAcpStore((s) => s.upsertSaved);
  const { start, testing } = useEndpointTest();
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [form, setForm] = useState<AcpEndpoint>(draft);
  const [fieldErrors, setFieldErrors] = useState<ReturnType<typeof validateEndpointForm> | null>(null);
  const idRef = useRef<string | null>(null);

  // 打开瞬间播种一次（渲染期状态调整，不落 effect）；发现面轮询仅弹窗打开期间运行
  const [seededOpen, setSeededOpen] = useState(false);
  if (open !== seededOpen) {
    setSeededOpen(open);
    if (open) {
      const seeded = seedForm(draft, consoleStatus);
      const candidates = targetOptions(directory, saved);
      if (!seeded.peer.trim() && candidates.length > 0) seeded.peer = candidates[0]!.peer;
      setForm(seeded);
      setFieldErrors(null);
      setAdvancedOpen(false);
    }
  }
  const ready = consoleStatus?.phase === "ready" ? consoleStatus : null;
  useDiscoveryPoll({
    statusUrl: open && ready ? (ready.statusUrl ?? null) : null,
    token: ready?.token ?? "",
  });

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
      token: form.token,
      peer: form.peer,
      alias: form.alias ?? "",
      adminUrl: form.adminUrl,
    });
    if (hasEndpointFormErrors(errors)) {
      setFieldErrors(errors);
      // 目标未选时展开高级区，错误与字段同屏（可观测，不静默）
      if (errors.peer) setAdvancedOpen(true);
      return false;
    }
    setFieldErrors(null);
    return true;
  };

  /** 连接类动作前置：目标必选（主字段下拉，禁自由文本留空连接）；错误显式呈现 */
  const requireTarget = (): boolean => {
    if (form.peer.trim()) return true;
    setFieldErrors({ peer: "targetRequired" });
    setAdvancedOpen(false);
    return false;
  };

  const test = () => {
    if (!validate() || !requireTarget()) return;
    start({ ...form, endpointId: ensureId() });
  };

  const save = () => {
    if (!validate()) return;
    const stamped = upsertSaved({ ...form, endpointId: ensureId() });
    onOpenChange(false);
    onSaved(stamped);
  };

  // UX3 一条龙：保存 + 测试连接（连接即测试）+ 连接 + 跳转会话，零二次点击
  const addAndOpen = () => {
    if (!validate() || !requireTarget()) return;
    const stamped = upsertSaved({ ...form, endpointId: ensureId() });
    onOpenChange(false);
    onSaved(stamped);
    start(stamped);
    navigate("/chat?agent=" + stamped.endpointId);
  };

  const history = wsUrlHistory(saved);
  const candidates = targetOptions(directory, saved);
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
            <Label htmlFor="contacts-endpoint-target">{t("contacts.endpoint.targetLabel")}</Label>
            <Select
              value={form.peer}
              onValueChange={(v) => patch("peer")(v)}
              disabled={testing}
            >
              <SelectTrigger id="contacts-endpoint-target" data-testid="contacts-endpoint-target">
                <SelectValue placeholder={t("contacts.endpoint.targetPlaceholder")} />
              </SelectTrigger>
              <SelectContent>
                {candidates.map((option) => (
                  <SelectItem key={option.peer} value={option.peer} data-testid={"contacts-endpoint-target-" + option.peer}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <EndpointFieldError code={fieldErrors?.peer} testidPrefix="contacts-endpoint-target-error" />
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
            <EndpointFieldError code={fieldErrors?.alias} />
          </div>
          <div className="flex flex-col gap-2 border-t pt-3">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="mr-auto gap-1 px-2"
              aria-expanded={advancedOpen}
              onClick={() => setAdvancedOpen((v) => !v)}
              data-testid="contacts-endpoint-advanced-toggle"
            >
              <ChevronDown aria-hidden className={"size-4 transition-transform " + (advancedOpen ? "" : "-rotate-90")} />
              {t("contacts.endpoint.advancedToggle")}
            </Button>
            <div hidden={!advancedOpen} data-testid="contacts-endpoint-advanced">
              <AdvancedFields
                form={form}
                fieldErrors={fieldErrors}
                patch={patch}
                testing={testing}
                history={history}
                historyEmptyLabel={t("contacts.endpoint.historyLabel")}
              />
            </div>
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
          <Button type="button" variant="outline" onClick={save} disabled={testing} data-testid="contacts-endpoint-save">
            {t("contacts.endpoint.save")}
          </Button>
          <Button type="button" onClick={addAndOpen} disabled={testing} data-testid="contacts-endpoint-add-open">
            {testing ? t("contacts.endpoint.opening") : t("contacts.endpoint.addAndOpen")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
