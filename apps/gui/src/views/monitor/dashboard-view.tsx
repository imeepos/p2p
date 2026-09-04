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

  // [&_[data-slot=card]]:h-full：同一 grid 行内卡片等高（状态卡 x4 与
  // 指标卡 x4 统一最小高度；底部双卡失衡同解），不触碰共享 StatCard。
  return (
    <div className="col-span-12 grid grid-cols-12 gap-4 [&_[data-slot=card]]:h-full">
      <PageHeader
        titleKey="dashboard.title"
        descriptionKey="dashboard.description"
      />
      <DashboardQuickActions />
      <DashboardStatusCards status={status} />
      <DashboardMetricCards metrics={metrics} />
      <DashboardTrendCard
        history={metricsHistory}
        running={status?.running ?? false}
      />
      <DegradeChainCard metrics={metrics} loading={metrics === null} />
      <RecentEventsCard events={events} loading={!subscriptionLive} />
    </div>
  );
}
