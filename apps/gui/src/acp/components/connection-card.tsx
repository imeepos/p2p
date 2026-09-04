import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { I18nKey } from "@/i18n/types";
import { useAcpStore } from "@/acp/acp-store";
import type { AcpEndpoint, AcpPhase } from "@/acp/protocol";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

const PHASE_TONE: Record<AcpPhase, StatusTone> = {
  idle: "neutral",
  connecting: "warning",
  online: "success",
  reconnecting: "warning",
  offline: "danger",
};

const PHASE_KEY: Record<AcpPhase, I18nKey> = {
  idle: "acp.connection.phase.idle",
  connecting: "acp.connection.phase.connecting",
  online: "acp.connection.phase.online",
  reconnecting: "acp.connection.phase.reconnecting",
  offline: "acp.connection.phase.offline",
};

const ERROR_KEY: Record<string, I18nKey> = {
  initializeFailed: "acp.errors.initializeFailed",
  endpointIncomplete: "acp.errors.endpointIncomplete",
  sessionNewFailed: "acp.errors.sessionNewFailed",
  sessionResumeFailed: "acp.errors.sessionResumeFailed",
  sessionCloseFailed: "acp.errors.sessionCloseFailed",
  promptFailed: "acp.errors.promptFailed",
};

const CLOSE_KEY: Record<string, I18nKey> = {
  denied: "acp.connection.closeDenied",
  "dial-failed": "acp.connection.closeDialFailed",
  abnormal: "acp.connection.closeAbnormal",
};

function Field(props: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  testid: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label>{props.label}</Label>
      <Input
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        data-testid={props.testid}
      />
    </div>
  );
}

function CloseInfoText() {
  const { t } = useTranslation();
  const closeInfo = useAcpStore((s) => s.closeInfo);
  if (!closeInfo || closeInfo.kind === "closed") return null;
  const key = CLOSE_KEY[closeInfo.kind];
  if (!key) return null;
  return (
    <p className="text-destructive text-sm" data-testid="acp-close-info">
      {t(key)}
      <span className="text-muted-foreground"> (code {closeInfo.code})</span>
    </p>
  );
}

function Notices() {
  const { t } = useTranslation();
  const reconnect = useAcpStore((s) => s.reconnect);
  const lastError = useAcpStore((s) => s.lastError);
  const errorKey = lastError ? ERROR_KEY[lastError] : null;
  return (
    <>
      <CloseInfoText />
      {reconnect ? (
        <p className="text-warning text-sm" data-testid="acp-reconnect-notice">
          {t("acp.connection.reconnectNotice", { attempt: reconnect.attempt, max: reconnect.max })}
        </p>
      ) : null}
      {errorKey ? (
        <p className="text-destructive text-sm" data-testid="acp-last-error">
          {t(errorKey)}
        </p>
      ) : null}
    </>
  );
}

function SavedEndpointChips() {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const setDraft = useAcpStore((s) => s.setDraft);
  const removeSaved = useAcpStore((s) => s.removeSaved);
  if (saved.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-muted-foreground text-xs">{t("acp.connection.savedTitle")}</span>
      {saved.map((endpoint) => (
        <span
          key={endpoint.peer + endpoint.wsUrl}
          className="bg-muted flex items-center gap-1 rounded-md px-2 py-0.5 text-xs"
        >
          <button
            type="button"
            onClick={() => setDraft(endpoint)}
            data-testid={"acp-endpoint-fill-" + endpoint.peer}
          >
            {endpoint.peer}
          </button>
          <button
            type="button"
            aria-label={t("acp.connection.remove")}
            onClick={() => removeSaved(endpoint.peer)}
            data-testid={"acp-endpoint-remove-" + endpoint.peer}
          >
            ×
          </button>
        </span>
      ))}
    </div>
  );
}

export function ConnectionCard() {
  const { t } = useTranslation();
  const phase = useAcpStore((s) => s.phase);
  const draft = useAcpStore((s) => s.draft);
  const setDraft = useAcpStore((s) => s.setDraft);
  const saveDraft = useAcpStore((s) => s.saveDraft);
  const connect = useAcpStore((s) => s.connect);
  const disconnect = useAcpStore((s) => s.disconnect);

  const patch = (field: keyof AcpEndpoint) => (v: string) => setDraft({ [field]: v });
  const connecting = phase !== "idle" && phase !== "offline";

  return (
    <Card data-testid="acp-connection-card">
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">{t("acp.connection.card")}</CardTitle>
        <span data-testid="acp-phase-badge">
          <StatusBadge tone={PHASE_TONE[phase]} dot>
            {t(PHASE_KEY[phase])}
          </StatusBadge>
        </span>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="grid gap-3 md:grid-cols-3">
          <Field label={t("acp.connection.wsUrl")} value={draft.wsUrl} onChange={patch("wsUrl")} testid="acp-input-ws-url" />
          <Field label={t("acp.connection.token")} value={draft.token} onChange={patch("token")} testid="acp-input-token" />
          <Field label={t("acp.connection.peer")} value={draft.peer} onChange={patch("peer")} testid="acp-input-peer" />
        </div>
        <Notices />
        <div className="flex flex-wrap gap-2">
          {connecting ? (
            <Button variant="outline" onClick={disconnect} data-testid="acp-disconnect">
              {t("acp.connection.disconnect")}
            </Button>
          ) : (
            <Button onClick={() => void connect()} data-testid="acp-connect">
              {t("acp.connection.connect")}
            </Button>
          )}
          <Button variant="ghost" onClick={saveDraft} data-testid="acp-save-endpoint">
            {t("acp.connection.save")}
          </Button>
        </div>
        <SavedEndpointChips />
      </CardContent>
    </Card>
  );
}
