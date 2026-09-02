import { PageHeader } from "@/components/page/page-header";
import { useNodeStore } from "@/stores/node-store";
import { DashboardMetricCards } from "./dashboard-metric-cards";
import { DashboardQuickActions } from "./dashboard-quick-actions";
import { DashboardStatusCards } from "./dashboard-status-cards";
import { DegradeChainCard } from "./degrade-chain-card";
import { RecentEventsCard } from "./recent-events-card";

// 仪表盘：快速操作 + 状态卡 x4 + 指标卡 x4 + 降级链成功率 + 最近事件。
export function DashboardView() {
  const status = useNodeStore((s) => s.status);
  const metrics = useNodeStore((s) => s.metrics);
  const events = useNodeStore((s) => s.events);
  const subscriptionLive = useNodeStore((s) => s.subscriptionLive);

  return (
    <>
      <PageHeader
        titleKey="dashboard.title"
        descriptionKey="dashboard.description"
      />
      <DashboardQuickActions />
      <DashboardStatusCards status={status} />
      <DashboardMetricCards metrics={metrics} />
      <DegradeChainCard metrics={metrics} loading={metrics === null} />
      <RecentEventsCard events={events} loading={!subscriptionLive} />
    </>
  );
}
