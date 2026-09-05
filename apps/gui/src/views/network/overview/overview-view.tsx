import { PageHeader } from "@/components/page/page-header";
import { useNodeStore } from "@/stores/node-store";
import { OverviewDialChainCard } from "./overview-dial-chain-card";
import { OverviewMetricCards } from "./overview-metric-cards";
import { OverviewRecentEventsCard } from "./overview-recent-events-card";
import { OverviewStatusCards } from "./overview-status-cards";
import { OverviewTrendCard } from "./overview-trend-card";
import { TroubleshootLinks } from "./troubleshoot-links";

// 网络概览页（docs/design/app-shell-redesign.md 4.2）：吸收旧仪表盘全部
// 信息卡，按「三秒看清 + 一跳排障」组织——节点状态卡（含启停）/ 四指标卡 /
// 10 分钟趋势 / 拨号跳成功率 / 最近事件 5 条 / 排障入口行。
export function OverviewView() {
  const status = useNodeStore((s) => s.status);
  const metrics = useNodeStore((s) => s.metrics);
  const metricsHistory = useNodeStore((s) => s.metricsHistory);
  const events = useNodeStore((s) => s.events);
  const subscriptionLive = useNodeStore((s) => s.subscriptionLive);
  const bootstrapPhase = useNodeStore((s) => s.bootstrapPhase);
  const linkFailed = !subscriptionLive && bootstrapPhase === "error";

  // 内层包裹只罩住两行状态/指标卡：[&_[data-slot=card]] 的选择器特异性
  // 高于卡片自身类，若罩全页会压掉底部卡的 min-h-40（D4），故收窄作用域。
  // h-full + min-h-28：两行卡等高且共享同一最小高度（IM-V2 D2）。
  return (
    <div className="col-span-12 grid grid-cols-12 gap-4">
      <PageHeader
        titleKey="network.overview.title"
        descriptionKey="network.overview.description"
      />
      <div className="col-span-12 grid grid-cols-12 gap-4 [&_[data-slot=card]]:h-full [&_[data-slot=card]]:min-h-28">
        <OverviewStatusCards status={status} />
        <OverviewMetricCards metrics={metrics} />
      </div>
      <OverviewTrendCard
        history={metricsHistory}
        running={status?.running ?? false}
      />
      <OverviewDialChainCard metrics={metrics} loading={metrics === null} />
      <OverviewRecentEventsCard
        events={events}
        loading={!subscriptionLive && bootstrapPhase !== "error"}
        linkFailed={linkFailed}
      />
      <TroubleshootLinks />
    </div>
  );
}
