import { useState } from "react";
import { useTranslation } from "react-i18next";
import { CircleCheck, CircleX, Loader2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { ACP_ERROR_KEYS } from "@/acp/store-events";
import type { I18nKey } from "@/i18n/types";
import type { AcpEndpoint } from "@/acp/protocol";
import { StatusBadge } from "@/views/shared/status-badge";

import {
  hasEndpointFormErrors,
  validateEndpointForm,
  type EndpointFormErrors,
} from "./endpoint-rules";
import { EndpointFieldError } from "./endpoint-advanced-fields";
import { useEndpointTest } from "./use-endpoint-test";

const PHASE_TONE = {
  idle: "neutral",
  connecting: "warning",
  online: "success",
  reconnecting: "warning",
  offline: "danger",
} as const;

const PHASE_KEY = {
  idle: "acp.connection.phase.idle",
  connecting: "acp.connection.phase.connecting",
  online: "acp.connection.phase.online",
  reconnecting: "acp.connection.phase.reconnecting",
  offline: "acp.connection.phase.offline",
} as const;

function Field(props: {
  id: string;
  label: string;
  value: string;
  onChange: (v: string) => void;
  testId: string;
  type?: "text" | "password";
  placeholder?: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={props.id}>{props.label}</Label>
      <Input
        id={props.id}
        type={props.type ?? "text"}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        placeholder={props.placeholder}
        data-testid={props.testId}
        autoComplete="off"
      />
    </div>
  );
}

// 连接块（§3.3 第 1 块）：endpoint 三字段只读展示 + 编辑态；「测试连接」；
// 连接态与最近错误显式呈现（失败路径不静默）。
export function DrawerConnection({
  endpoint,
  isActive,
  onSave,
}: {
  endpoint: AcpEndpoint;
  isActive: boolean;
  onSave: (endpoint: AcpEndpoint) => void;
}) {
  const { t } = useTranslation();
  const phase = useAcpStore((s) => s.phase);
  const lastError = useAcpStore((s) => s.lastError);
  const closeInfo = useAcpStore((s) => s.closeInfo);
  const endpointId = endpoint.endpointId!;
  const lastTest = useEndpointMetaStore((s) => s.lastTest[endpointId] ?? null);
  const { start, testing } = useEndpointTest();
  const [editing, setEditing] = useState(false);
  const [draft, setDraftLocal] = useState<AcpEndpoint>(endpoint);
  const [errors, setErrors] = useState<EndpointFormErrors | null>(null);

  const phaseKey = PHASE_KEY[isActive ? phase : "idle"] as I18nKey;
  const outcome = testing ? null : lastTest;

  const startTest = () => start(editing ? draft : endpoint);

  // P2#7 取消编辑：丢弃草稿恢复原 endpoint 值并退出编辑态
  const cancelEdit = () => {
    setDraftLocal(endpoint);
    setErrors(null);
    setEditing(false);
  };

  // P2#6 保存前同款预校验（endpoint-add-dialog save 路径口径：token 可空），
  // 非法则行内报错不保存
  const save = () => {
    const next = validateEndpointForm(
      {
        wsUrl: draft.wsUrl,
        token: draft.token,
        peer: draft.peer,
        alias: draft.alias ?? "",
        adminUrl: draft.adminUrl,
      },
      { requireToken: false },
    );
    if (hasEndpointFormErrors(next)) {
      setErrors(next);
      return;
    }
    setErrors(null);
    onSave({ ...endpoint, ...draft });
    setEditing(false);
  };

  return (
    <div className="flex flex-col gap-3" data-testid="contacts-drawer-connection">
      <div className="flex items-center justify-between gap-2">
        <span data-testid="contacts-drawer-phase">
          <StatusBadge tone={PHASE_TONE[isActive ? phase : "idle"]} dot>
            {t(phaseKey)}
          </StatusBadge>
        </span>
        {editing ? (
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={cancelEdit}
              data-testid="contacts-drawer-cancel"
            >
              {t("common.actions.cancel")}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={save}
              data-testid="contacts-drawer-save"
            >
              {t("contacts.drawer.save")}
            </Button>
          </div>
        ) : (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              setDraftLocal(endpoint);
              setErrors(null);
              setEditing(true);
            }}
            data-testid="contacts-drawer-edit"
          >
            {t("contacts.drawer.edit")}
          </Button>
        )}
      </div>
      {editing ? (
        <div className="flex flex-col gap-2">
          <Field
            id="drawer-wsurl"
            label={t("contacts.endpoint.wsUrlLabel")}
            value={draft.wsUrl}
            onChange={(v) => setDraftLocal((p) => ({ ...p, wsUrl: v }))}
            testId="contacts-drawer-wsurl"
            placeholder={t("contacts.endpoint.wsUrlPlaceholder")}
          />
          <EndpointFieldError code={errors?.wsUrl} testidPrefix="contacts-drawer-error" />
          <Field
            id="drawer-token"
            label={t("contacts.endpoint.tokenLabel")}
            value={draft.token}
            onChange={(v) => setDraftLocal((p) => ({ ...p, token: v }))}
            testId="contacts-drawer-token"
            type="password"
            placeholder={t("contacts.endpoint.tokenPlaceholder")}
          />
          <EndpointFieldError code={errors?.token} testidPrefix="contacts-drawer-error" />
          <Field
            id="drawer-peer"
            label={t("contacts.endpoint.peerLabel")}
            value={draft.peer}
            onChange={(v) => setDraftLocal((p) => ({ ...p, peer: v }))}
            testId="contacts-drawer-peer"
            placeholder={t("contacts.endpoint.peerPlaceholder")}
          />
          <EndpointFieldError code={errors?.peer} testidPrefix="contacts-drawer-error" />
        </div>
      ) : (
        <div className="flex flex-col gap-1 text-sm">
          <p className="truncate">
            <span className="text-muted-foreground">{t("contacts.endpoint.wsUrlLabel")}：</span>
            <span className="font-mono text-xs">{endpoint.wsUrl}</span>
          </p>
          <p className="truncate">
            <span className="text-muted-foreground">{t("contacts.endpoint.peerLabel")}：</span>
            <span className="font-mono text-xs">{endpoint.peer || "—"}</span>
          </p>
        </div>
      )}
      <div className="flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={startTest}
          disabled={testing}
          data-testid="contacts-drawer-test"
        >
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
          <span className="inline-flex items-center gap-1 text-success text-xs" data-testid="contacts-drawer-test-ok">
            <CircleCheck aria-hidden className="size-3.5" />
            {t("contacts.endpoint.testPassed")}
          </span>
        ) : null}
        {outcome === "failed" ? (
          <span className="inline-flex items-center gap-1 text-destructive text-xs" data-testid="contacts-drawer-test-failed">
            <CircleX aria-hidden className="size-3.5" />
            {t("contacts.endpoint.testFailed")}
          </span>
        ) : null}
      </div>
      {isActive && closeInfo && closeInfo.kind !== "closed" ? (
        <p className="text-destructive text-xs" data-testid="contacts-drawer-close-info">
          {t("contacts.drawer.lastError")}：{closeInfo.kind} (code {closeInfo.code})
        </p>
      ) : null}
      {isActive && lastError ? (
        <p className="text-destructive text-xs" data-testid="contacts-drawer-last-error">
          {t("contacts.drawer.lastError")}：
          {ACP_ERROR_KEYS[lastError]
            ? t(ACP_ERROR_KEYS[lastError])
            : "[" + lastError + "]"}
        </p>
      ) : null}
      {!isActive && !lastError && !closeInfo ? (
        <p className="text-muted-foreground text-xs" data-testid="contacts-drawer-no-error">
          {t("contacts.drawer.noError")}
        </p>
      ) : null}
    </div>
  );
}
