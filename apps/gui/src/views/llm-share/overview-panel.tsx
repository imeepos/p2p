import { Activity, ArrowDownRight, ArrowUpRight, FileText, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { LucideIcon } from "lucide-react";

import type { Locale } from "@/i18n";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDateTime } from "@/lib/format";
import { errorText } from "@/views/shared/form-flow";

import { subscribeLedgerMutated } from "./ledger-sync";
import { isOfferNotPublished, warnOfferLoadOnce } from "./offer-errors";
import { OfferStatusCard } from "./offer-status-card";
import { recentEntries, summarizeLedger, type LlmOverviewStats } from "./overview-stats";
import type { LlmLedgerEntry, LlmOfferView, LlmServeStatus, LlmShareBackend } from "./types";

/** 概览页快捷跳转目标（借用 / 能力发布 / 账本明细） */
export type OverviewGoTab = "borrow" | "offer" | "ledger";

interface OverviewData {
  offer: LlmOfferView | null;
  offerError: string | null;
  allowCount: number | null;
  providerCount: number | null;
  serve: LlmServeStatus | null;
  entries: LlmLedgerEntry[];
  stats: LlmOverviewStats | null;
}

const EMPTY_DATA: OverviewData = {
  offer: null,
  offerError: null,
  allowCount: null,
  providerCount: null,
  serve: null,
  entries: [],
  stats: null,
};

function StatCard({
  icon: Icon,
  label,
  value,
  testid,
}: {
  icon: LucideIcon;
  label: string;
  value: number;
  testid: string;
}) {
  return (
    <Card data-testid={testid}>
      <CardContent className="flex items-center gap-3 p-4">
        <Icon aria-hidden className="text-muted-foreground size-4 shrink-0" />
        <div className="min-w-0">
          <p className="text-muted-foreground text-xs">{label}</p>
          <p className="truncate font-mono text-lg font-semibold tabular-nums" data-testid={`${testid}-value`}>
            {value}
          </p>
        </div>
      </CardContent>
    </Card>
  );
}

// 最近流水行：概览只露时间/模型/tokens 摘要，对端与过滤明细留在账本 tab。
function RecentRow({ entry }: { entry: LlmLedgerEntry }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  return (
    <div data-testid="overview-recent-row" className="flex items-center gap-2 rounded-md border p-2 text-xs">
      <span className="text-muted-foreground shrink-0">{formatDateTime(entry.ts * 1000, locale)}</span>
      <span className="min-w-0 flex-1 truncate font-medium">{entry.model}</span>
      {entry.estimated ? (
        <span className="text-muted-foreground">{t("llmShare.ledger.estimatedFlag")}</span>
      ) : null}
      <span className="font-mono tabular-nums">{entry.tokens}</span>
    </div>
  );
}

function ResourceCell({
  label,
  value,
  testid,
}: {
  label: string;
  value: string;
  testid: string;
}) {
  return (
    <div className="rounded-md border p-3" data-testid={testid}>
      <p className="text-muted-foreground text-xs">{label}</p>
      <p className="mt-1 text-sm font-medium">{value}</p>
    </div>
  );
}

// 「概览」tab（tab 化后的统计分析首页）：统计卡 + 声明状态 + 最近流水 +
// 资源计数，全部只读；写路径动作经快捷按钮跳对应 tab（层次感：首页只给数字与入口）。
export function OverviewPanel({
  backend,
  onGoTab,
}: {
  backend: LlmShareBackend;
  onGoTab: (tab: OverviewGoTab) => void;
}) {
  const { t } = useTranslation();
  const [data, setData] = useState<OverviewData>(EMPTY_DATA);
  const [loaded, setLoaded] = useState(false);

  const load = useCallback(async () => {
    // 六路并行、各自兜底：未发布 offer 是常态非故障（静默）；其余失败留
    // console 告警且计数置 null（前端显示 -），绝不静默吞成 0。
    // settle 包一层：mock/桥接实现可能同步 throw，直接 .catch 挂不上。
    const settle = async <T,>(call: () => Promise<T>): Promise<T | Error> => {
      try {
        return await call();
      } catch (error) {
        return error instanceof Error ? error : new Error(String(error));
      }
    };
    const orNull = <T,>(res: T | Error, label: string): T | null => {
      if (!(res instanceof Error)) return res;
      console.warn(`[llm-share] overview ${label} 读取失败`, res);
      return null;
    };
    const [offer, allowRes, providersRes, serveRes, entriesRes, balanceRes] = await Promise.all([
      settle(() => backend.offerShow()),
      settle(() => backend.allowList()),
      settle(() => backend.providerList()),
      settle(() => backend.serveStatus()),
      settle(() => backend.ledgerList()),
      settle(() => backend.ledgerBalance()),
    ]);
    const allow = orNull(allowRes, "allowlist");
    const providers = orNull(providersRes, "providers");
    const serve = orNull(serveRes, "serve");
    const entries = orNull(entriesRes, "ledger");
    const balance = orNull(balanceRes, "balance");
    const offerError =
      offer instanceof Error && !isOfferNotPublished(offer)
        ? (warnOfferLoadOnce(offer), errorText(offer))
        : null;
    const ledgerRows = Array.isArray(entries) ? entries : [];
    setData({
      offer: offer instanceof Error ? null : offer,
      offerError,
      allowCount: allow ? allow.entries.length : null,
      providerCount: providers ? providers.providers.length : null,
      serve: serve ?? null,
      entries: ledgerRows,
      stats: Array.isArray(balance) ? summarizeLedger(ledgerRows, balance) : null,
    });
  }, [backend]);

  const loadRef = useRef(load);
  useEffect(() => {
    loadRef.current = load;
  });

  // 挂载首拉 + 借用入账联动重拉（R2-01 同源事件）
  useEffect(() => {
    void (async () => {
      await loadRef.current();
      setLoaded(true);
    })();
  }, []);

  useEffect(() => subscribeLedgerMutated(() => void loadRef.current()), []);

  const stats = data.stats;
  const recent = recentEntries(data.entries, 5);

  return (
    <div className="flex flex-col gap-4" data-testid="overview-panel">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">{t("llmShare.overview.statsTitle")}</h2>
        <Button type="button" size="sm" variant="ghost" onClick={() => void load()} data-testid="overview-refresh">
          <RefreshCw aria-hidden className="size-3.5" />
          {t("llmShare.overview.refresh")}
        </Button>
      </div>
      {stats ? (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          <StatCard icon={FileText} label={t("llmShare.overview.statEntries")} value={stats.entriesCount} testid="stat-entries" />
          <StatCard icon={Activity} label={t("llmShare.overview.statTokens")} value={stats.totalTokens} testid="stat-tokens" />
          <StatCard icon={ArrowUpRight} label={t("llmShare.overview.statLentOut")} value={stats.lentOut} testid="stat-lent" />
          <StatCard icon={ArrowDownRight} label={t("llmShare.overview.statBorrowed")} value={stats.borrowed} testid="stat-borrowed" />
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button type="button" size="sm" onClick={() => onGoTab("borrow")} data-testid="overview-go-borrow">
          {t("llmShare.overview.goBorrow")}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={() => onGoTab("ledger")} data-testid="overview-go-ledger">
          {t("llmShare.overview.goLedger")}
        </Button>
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Card data-testid="overview-offer">
          <CardHeader>
            <CardTitle className="text-sm">{t("llmShare.panels.offer")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {data.offer ? (
              <OfferStatusCard offer={data.offer} />
            ) : (
              <>
                {data.offerError ? <p role="alert" className="text-destructive text-xs">{data.offerError}</p> : null}
                {loaded ? (
                  <>
                    <p className="text-muted-foreground text-xs">{t("llmShare.offer.emptyTitle")}</p>
                    <Button type="button" size="sm" className="w-fit" onClick={() => onGoTab("offer")} data-testid="overview-go-offer">
                      {t("llmShare.overview.goOffer")}
                    </Button>
                  </>
                ) : null}
              </>
            )}
          </CardContent>
        </Card>
        <Card data-testid="overview-recent">
          <CardHeader>
            <CardTitle className="text-sm">{t("llmShare.overview.recentTitle")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {recent.length === 0 ? (
              <p className="text-muted-foreground text-xs">{t("llmShare.ledger.emptyList")}</p>
            ) : (
              recent.map((row) => <RecentRow key={row.reqId} entry={row} />)
            )}
          </CardContent>
        </Card>
      </div>
      <div className="grid gap-3 sm:grid-cols-3" data-testid="overview-resources">
        <ResourceCell
          label={t("llmShare.overview.allowlistLabel")}
          value={data.allowCount === null ? "-" : t("llmShare.overview.allowlistValue", { count: data.allowCount })}
          testid="overview-allow"
        />
        <ResourceCell
          label={t("llmShare.overview.providersLabel")}
          value={data.providerCount === null ? "-" : t("llmShare.overview.providersValue", { count: data.providerCount })}
          testid="overview-providers"
        />
        <ResourceCell
          label={t("llmShare.overview.serveLabel")}
          value={
            data.serve === null
              ? "-"
              : data.serve.assembled
                ? t("llmShare.serve.assembled")
                : t("llmShare.serve.notAssembled")
          }
          testid="overview-serve"
        />
      </div>
    </div>
  );
}
