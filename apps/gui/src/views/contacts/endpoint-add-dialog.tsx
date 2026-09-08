import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { ChevronDown, CircleCheck, CircleX, Loader2Icon } from "lucide-react";

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
import { newEndpointId } from "@/acp/endpoint-storage";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import { findShareLinkInText } from "@/acp/share-model";
import { useDiscoveryPoll } from "@/acp/use-discovery-poll";

import { hasEndpointFormErrors, seedForm, shareEndpointId, targetOptions, validateEndpointForm, wsUrlHistory } from "./endpoint-rules";
import { AdvancedFields, EndpointFieldError, EndpointTargetPicker, WsUrlField } from "./endpoint-advanced-fields";
import { EndpointShareImport } from "./endpoint-share-import";
import { useEndpointTest } from "./use-endpoint-test";

interface EndpointAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (endpoint: AcpEndpoint) => void;
}

/** 添加 agent endpoint（§3.2/§3.4 + UX3 收敛）：wsUrl 为主字段（历史值下拉），
 *  目标节点从发现清单下拉选择；token/peer/分享管理收进「高级」折叠，默认以本机
 *  console 值预填（本机 agent 由托管自动接入，弹窗顶部场景提示引导远端场景）。
 *  token 分径校验：保存可空（先存后连），连接类动作必填（2026-09-07 用户裁决）。
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
  // wsUrl 字段识别到分享链接时的导入态（§8）：非 null 即切导入流
  const [shareLink, setShareLink] = useState<string | null>(null);
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
      setShareLink(null);
    }
  }
  const ready = consoleStatus?.phase === "connected" ? consoleStatus : null;
  useDiscoveryPoll({
    statusUrl: open && ready ? (ready.statusUrl ?? null) : null,
    token: ready?.token ?? "",
  });

  const patch = (field: keyof AcpEndpoint) => (value: string) => {
    // wsUrl 字段识别到分享链接（§8 guest 导入）：整段粘贴即切入导入流，
    // 不落手工表单（链接不是 ws:// URL，落表单只会吃到格式错误）
    if (field === "wsUrl") {
      const link = findShareLinkInText(value);
      if (link) {
        setShareLink(link);
        setFieldErrors(null);
        return;
      }
    }
    setForm((prev) => ({ ...prev, [field]: value }));
    setFieldErrors(null);
  };

  const ensureId = (): string => {
    if (!idRef.current) idRef.current = form.endpointId?.trim() || newEndpointId();
    return idRef.current;
  };

  /** 分径校验：保存路径 token 可空（先存后连）；连接类动作必填。错误显式
   *  呈现且与字段同屏（peer/token 在折叠区时自动展开，可观测，不静默） */
  const validate = (requireToken: boolean): boolean => {
    const errors = validateEndpointForm(
      {
        wsUrl: form.wsUrl,
        token: form.token,
        peer: form.peer,
        alias: form.alias ?? "",
        adminUrl: form.adminUrl,
      },
      { requireToken },
    );
    if (hasEndpointFormErrors(errors)) {
      setFieldErrors(errors);
      if (errors.peer || errors.token) setAdvancedOpen(true);
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
    if (!validate(true) || !requireTarget()) return;
    start({ ...form, endpointId: ensureId() });
  };

  const save = () => {
    if (!validate(false)) return;
    const stamped = upsertSaved({ ...form, endpointId: ensureId() });
    toastSuccess(t("contacts.endpoint.saveSuccess"));
    onOpenChange(false);
    onSaved(stamped);
  };

  // UX3 一条龙：保存 + 测试连接（连接即测试）+ 连接 + 跳转会话，零二次点击
  const addAndOpen = () => {
    if (!validate(true) || !requireTarget()) return;
    const stamped = upsertSaved({ ...form, endpointId: ensureId() });
    onOpenChange(false);
    onSaved(stamped);
    start(stamped);
    navigate("/chat?agent=" + stamped.endpointId);
  };

  // 本机连接面（console ready 时直传导入流，免经 store draft 中转）
  const shareConn = ready?.wsUrl && ready.token
    ? { statusUrl: ready.statusUrl ?? "", token: ready.token }
    : null;

  /** 分享导入成功（§8）：按分享 peer 落 saved endpoint（稳定 id 幂等重导
   *  入即刷新）；连接面 = 本机 console，凭据在 console 策略表，对话经本机
   *  agent 桥接拨号到对方（ws peer 参数语义）。 */
  const onShareJoined = (peer: string) => {
    const stamped = upsertSaved({
      endpointId: shareEndpointId(peer),
      peer,
      wsUrl: form.wsUrl,
      token: form.token,
      statusUrl: form.statusUrl,
      adminUrl: form.adminUrl,
      alias: form.alias ?? "",
    });
    onOpenChange(false);
    onSaved(stamped);
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
          {ready ? (
            <p className="text-muted-foreground text-xs" data-testid="contacts-endpoint-local-hint">
              {t("contacts.endpoint.localAgentHint")}
            </p>
          ) : null}
          {shareLink ? (
            <EndpointShareImport
              link={shareLink}
              conn={shareConn}
              onJoined={onShareJoined}
              onExit={() => setShareLink(null)}
            />
          ) : (
            <>
          <WsUrlField
            form={form}
            fieldErrors={fieldErrors}
            patch={patch}
            testing={testing}
            history={history}
            historyEmptyLabel={t("contacts.endpoint.historyLabel")}
          />
          <EndpointTargetPicker
            form={form}
            candidates={candidates}
            fieldErrors={fieldErrors}
            patch={patch}
          />
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-alias">{t("contacts.endpoint.aliasLabel")}</Label>
            <Input
              id="contacts-endpoint-alias"
              value={form.alias ?? ""}
              onChange={(e) => patch("alias")(e.target.value)}
              placeholder={t("contacts.endpoint.aliasPlaceholder")}
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
              <AdvancedFields form={form} fieldErrors={fieldErrors} patch={patch} />
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
              <span className="inline-flex items-center gap-1 text-success text-xs" data-testid="contacts-endpoint-test-ok">
                <CircleCheck aria-hidden className="size-3.5" />
                {t("contacts.endpoint.testPassed")}
              </span>
            ) : null}
            {outcome === "failed" ? (
              <span className="inline-flex items-center gap-1 text-destructive text-xs" data-testid="contacts-endpoint-test-failed">
                <CircleX aria-hidden className="size-3.5" />
                {t("contacts.endpoint.testFailed")}
              </span>
            ) : null}
          </div>
            </>
          )}
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
          {!shareLink ? (
            <>
              <Button type="button" variant="outline" onClick={save} disabled={testing} data-testid="contacts-endpoint-save">
                {t("contacts.endpoint.save")}
              </Button>
              <Button type="button" onClick={addAndOpen} disabled={testing} data-testid="contacts-endpoint-add-open">
                {testing ? t("contacts.endpoint.opening") : t("contacts.endpoint.addAndOpen")}
              </Button>
            </>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
