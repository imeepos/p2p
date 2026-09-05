import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAcpStore } from "@/acp/acp-store";
import { ACP_ERROR_KEYS } from "@/acp/store-events";
import type { I18nKey } from "@/i18n/types";
import type { AcpEndpoint } from "@/acp/protocol";
import { StatusBadge } from "@/views/shared/status-badge";

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
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={props.id}>{props.label}</Label>
      <Input
        id={props.id}
        type={props.type ?? "text"}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
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
  const { start, testingId, result } = useEndpointTest();
  const [editing, setEditing] = useState(false);
  const [draft, setDraftLocal] = useState<AcpEndpoint>(endpoint);

  const endpointId = endpoint.endpointId!;
  const testing = testingId === endpointId;
  const phaseKey = PHASE_KEY[isActive ? phase : "idle"] as I18nKey;

  const startTest = () => start(endpointId, editing ? draft : endpoint);

  return (
    <div className="flex flex-col gap-3" data-testid="contacts-drawer-connection">
      <div className="flex items-center justify-between gap-2">
        <span data-testid="contacts-drawer-phase">
          <StatusBadge tone={PHASE_TONE[isActive ? phase : "idle"]} dot>
            {t(phaseKey)}
          </StatusBadge>
        </span>
        {editing ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              onSave({ ...endpoint, ...draft });
              setEditing(false);
            }}
            data-testid="contacts-drawer-save"
          >
            {t("contacts.drawer.save")}
          </Button>
        ) : (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              setDraftLocal(endpoint);
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
          />
          <Field
            id="drawer-token"
            label={t("contacts.endpoint.tokenLabel")}
            value={draft.token}
            onChange={(v) => setDraftLocal((p) => ({ ...p, token: v }))}
            testId="contacts-drawer-token"
            type="password"
          />
          <Field
            id="drawer-peer"
            label={t("contacts.endpoint.peerLabel")}
            value={draft.peer}
            onChange={(v) => setDraftLocal((p) => ({ ...p, peer: v }))}
            testId="contacts-drawer-peer"
          />
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
          {t("contacts.endpoint.test")}
        </Button>
        {result && result.id === endpointId && result.outcome === "ok" ? (
          <span className="text-success text-xs" data-testid="contacts-drawer-test-ok">
            {t("contacts.endpoint.testPassed")}
          </span>
        ) : null}
        {result && result.id === endpointId && result.outcome === "failed" ? (
          <span className="text-destructive text-xs" data-testid="contacts-drawer-test-failed">
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
