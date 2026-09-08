import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";

import type { I18nKey } from "@/i18n/types";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { PageHeader } from "@/components/page/page-header";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { AllowlistPanel } from "./allowlist-panel";
import { BorrowPanel } from "./borrow-panel";
import { hasBorrowPrefill } from "./borrow-prefill";
import { LedgerPanel } from "./ledger-panel";
import { OfferPanel } from "./offer-panel";
import { OverviewPanel } from "./overview-panel";
import { ProviderPanel } from "./provider-panel";
import type { LlmShareBackend } from "./types";

export type LlmShareTab =
  | "overview"
  | "borrow"
  | "ledger"
  | "offer"
  | "allowlist"
  | "providers";

// 常用的离用户近：统计概览（首页）→ 借用（高频动作）→ 账本（高频查询）
// → 能力发布/白名单（出借方管理）→ 上游配置（低频配置）。
const TAB_ORDER: readonly LlmShareTab[] = [
  "overview",
  "borrow",
  "ledger",
  "offer",
  "allowlist",
  "providers",
];

const TAB_LABEL: Record<LlmShareTab, I18nKey> = {
  overview: "llmShare.tabs.overview",
  borrow: "llmShare.tabs.borrow",
  ledger: "llmShare.tabs.ledger",
  offer: "llmShare.tabs.offer",
  allowlist: "llmShare.tabs.allowlist",
  providers: "llmShare.tabs.providers",
};

// 初始落点：显式 ?tab= 深链优先；chat 兑换跳转（query peer/model）或存在
// borrow 预填交接时直接落借用 tab；其余默认概览（统计分析首页）。
function initialTab(params: URLSearchParams): LlmShareTab {
  const tab = params.get("tab");
  if (tab !== null && (TAB_ORDER as readonly string[]).includes(tab)) {
    return tab as LlmShareTab;
  }
  if (params.get("peer") !== null || params.get("model") !== null || hasBorrowPrefill()) {
    return "borrow";
  }
  return "overview";
}

const TRIGGER_ACTIVE_CLS =
  "data-[state=active]:border-primary data-[state=active]:bg-primary data-[state=active]:text-primary-foreground dark:data-[state=active]:border-primary dark:data-[state=active]:bg-primary dark:data-[state=active]:text-primary-foreground";

// /llm-share 六 tab 页（原五面板挤一屏 → tab 分类）：概览为统计分析首页，
// 其余 tab 各自是列表页或配置页；Radix Tabs 非激活内容不挂载，一次只看一屏。
// 自持一份 ConfirmProvider（borrow 二次确认依赖，acp-view 先例）。
export function LlmShareView({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const [tab, setTab] = useState<LlmShareTab>(() => initialTab(searchParams));

  const goTab = (next: LlmShareTab) => {
    setTab(next);
    const params = new URLSearchParams(searchParams);
    params.set("tab", next);
    setSearchParams(params, { replace: true });
  };

  return (
    <ConfirmProvider>
      <div className="flex min-h-0 flex-col gap-4">
        <PageHeader titleKey="llmShare.title" descriptionKey="llmShare.description" />
        <Tabs
          value={tab}
          onValueChange={(value) => goTab(value as LlmShareTab)}
          className="gap-4"
        >
          <TabsList className="h-auto flex-wrap">
            {TAB_ORDER.map((key) => (
              <TabsTrigger key={key} value={key} className={TRIGGER_ACTIVE_CLS}>
                {t(TAB_LABEL[key])}
              </TabsTrigger>
            ))}
          </TabsList>
          <TabsContent value="overview" className="mt-0">
            <OverviewPanel backend={backend} onGoTab={goTab} />
          </TabsContent>
          <TabsContent value="borrow" className="mt-0">
            <BorrowPanel backend={backend} />
          </TabsContent>
          <TabsContent value="ledger" className="mt-0">
            <LedgerPanel backend={backend} />
          </TabsContent>
          <TabsContent value="offer" className="mt-0">
            <OfferPanel backend={backend} />
          </TabsContent>
          <TabsContent value="allowlist" className="mt-0">
            <AllowlistPanel backend={backend} />
          </TabsContent>
          <TabsContent value="providers" className="mt-0">
            <ProviderPanel backend={backend} />
          </TabsContent>
        </Tabs>
      </div>
    </ConfirmProvider>
  );
}
