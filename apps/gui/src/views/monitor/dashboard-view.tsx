import { PageHeader } from "@/components/page/page-header";
import { useNodeStore } from "@/stores/node-store";
import { DashboardMetricCards } from "./dashboard-metric-cards";
import { DashboardQuickActions } from "./dashboard-quick-actions";
import { DashboardStatusCards } from "./dashboard-status-cards";
import { DashboardTrendCard } from "./dashboard-trend-card";
import { DegradeChainCard } from "./degrade-chain-card";
import { RecentEventsCard } from "./recent-events-card";

// 仪表盘：快速操作 + 状态卡 x4 + 指标卡 x4 + 趋势卡 + 降级链成功率 + 最近事件。
export function DashboardView() {
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
        titleKey="dashboard.title"
        descriptionKey="dashboard.description"
      />
      <DashboardQuickActions />
      <div className="col-span-12 grid grid-cols-12 gap-4 [&_[data-slot=card]]:h-full [&_[data-slot=card]]:min-h-28">
        <DashboardStatusCards status={status} />
        <DashboardMetricCards metrics={metrics} />
      </div>
      <DashboardTrendCard
        history={metricsHistory}
        running={status?.running ?? false}
      />
      <DegradeChainCard metrics={metrics} loading={metrics === null} />
      <RecentEventsCard
        events={events}
        loading={!subscriptionLive && bootstrapPhase !== "error"}
        linkFailed={linkFailed}
      />
    </div>
  );
}
