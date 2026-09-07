import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";
import type { Locale } from "@/i18n";
import { formatUptime } from "@/lib/format";

import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

import type { LlmOfferStatus, LlmOfferView } from "./types";

// §16.2-5：expired/not_yet_valid = 常态中性；peer_mismatch/bad_signature =
// 醒目警示（danger 徽章 + role=alert + destructive 描边）。
const STATUS_TONE: Record<LlmOfferStatus, StatusTone> = {
  live: "success",
  expired: "neutral",
  not_yet_valid: "neutral",
  peer_mismatch: "danger",
  bad_signature: "danger",
};

const STATUS_KEY: Record<LlmOfferStatus, I18nKey> = {
  live: "llmShare.offer.statusLive",
  expired: "llmShare.offer.statusExpired",
  not_yet_valid: "llmShare.offer.statusNotYetValid",
  peer_mismatch: "llmShare.offer.statusPeerMismatch",
  bad_signature: "llmShare.offer.statusBadSignature",
};

function OfferFieldRow({ label, value, testId }: { label: string; value: string; testId?: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="truncate font-mono" title={value} data-testid={testId}>
        {value}
      </dd>
    </>
  );
}

export function OfferStatusCard({ offer }: { offer: LlmOfferView }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const warning = offer.status === "peer_mismatch" || offer.status === "bad_signature";
  const spare = Object.entries(offer.spare)
    .map(([m, n]) => `${m}=${n}`)
    .join(", ");
  // R2-09：剩余时间人性化——live 显时长、过期/未生效显状态语义，不再裸秒
  const remaining =
    offer.status === "expired"
      ? t("llmShare.offer.statusExpired")
      : offer.status === "not_yet_valid"
        ? t("llmShare.offer.statusNotYetValid")
        : formatUptime(offer.remainingSecs, locale);
  return (
    <div
      data-testid="offer-status"
      data-status={offer.status}
      className={
        warning
          ? "border-destructive/40 rounded-md border p-3 text-sm"
          : "rounded-md border p-3 text-sm"
      }
    >
      <div className="flex items-center gap-2">
        <StatusBadge tone={STATUS_TONE[offer.status]} dot>
          {t(STATUS_KEY[offer.status])}
        </StatusBadge>
        <span className="text-muted-foreground text-xs">
          {t("llmShare.offer.labelStatus")}
        </span>
      </div>
      {warning ? (
        <p role="alert" className="text-destructive mt-2 text-xs">
          {t("llmShare.offer.securityWarning")}
        </p>
      ) : null}
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <OfferFieldRow label={t("llmShare.offer.labelPeer")} value={offer.peer} />
        <OfferFieldRow
          label={t("llmShare.offer.labelModels")}
          value={offer.models.join(", ")}
        />
        <OfferFieldRow label={t("llmShare.offer.labelSpare")} value={spare} />
        <OfferFieldRow
          label={t("llmShare.offer.labelPeriodEnds")}
          value={offer.periodEnds}
        />
        <OfferFieldRow
          label={t("llmShare.offer.labelRemaining")}
          value={remaining}
          testId="offer-remaining"
        />
      </dl>
    </div>
  );
}
