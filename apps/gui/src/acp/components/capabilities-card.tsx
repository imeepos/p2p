import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { I18nKey } from "@/i18n/types";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";
import { useAcpStore } from "@/acp/acp-store";
import type { AcpPhase } from "@/acp/protocol";

type CapValue = boolean | undefined;

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

/** 能力位如实渲染：agent 未声明显示 undeclared，绝不代答 true/false */
function CapBadge({ label, value }: { label: string; value: CapValue }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-2 text-sm">
      <span>{label}</span>
      {value === undefined ? (
        <StatusBadge tone="neutral">{t("acp.capabilities.undeclared")}</StatusBadge>
      ) : value ? (
        <StatusBadge tone="success">{t("acp.capabilities.supported")}</StatusBadge>
      ) : (
        <StatusBadge tone="neutral">{t("acp.capabilities.unsupported")}</StatusBadge>
      )}
    </div>
  );
}

export function CapabilitiesCard() {
  const { t } = useTranslation();
  const capabilities = useAcpStore((s) => s.capabilities);
  const phase = useAcpStore((s) => s.phase);
  if (!capabilities) return null;
  const caps = capabilities.agentCapabilities ?? {};
  const agent = capabilities.agentInfo;
  const undeclared = t("acp.capabilities.undeclared");
  return (
    <Card data-testid="acp-capabilities-card">
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">{t("acp.capabilities.card")}</CardTitle>
        <span data-testid="acp-phase-badge">
          <StatusBadge tone={PHASE_TONE[phase]} dot>
            {t(PHASE_KEY[phase])}
          </StatusBadge>
        </span>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2 text-sm">
          <span>{t("acp.capabilities.agent")}</span>
          <span className="text-muted-foreground truncate">
            {agent?.name ?? undeclared}
            {agent?.version ? " v" + agent.version : ""}
          </span>
        </div>
        <div className="flex items-center justify-between gap-2 text-sm">
          <span>{t("acp.capabilities.protocol")}</span>
          <span className="text-muted-foreground">
            {capabilities.protocolVersion ?? undeclared}
          </span>
        </div>
        <CapBadge label={t("acp.capabilities.loadSession")} value={caps.loadSession} />
        <CapBadge
          label={t("acp.capabilities.embeddedContext")}
          value={caps.promptCapabilities?.embeddedContext}
        />
      </CardContent>
    </Card>
  );
}
