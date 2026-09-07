// /acp-manage 本地 ACP 管理页（信息架构：rail 不动，命令面板与 ACP 视图入口可达）。
// 三面板：本机 agent 服务状态 / 工作区管理（admin 实时增删）/ 会话管理（复用连接态）。
import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { ServiceStatusCard } from "./service-status-card";
import { SessionsCard } from "./sessions-card";
import { WorkspaceManageCard } from "./workspace-manage-card";

export function AcpManageView() {
  const { t } = useTranslation();
  return (
    <div className="grid grid-cols-12 gap-4 p-4" data-testid="acp-manage-view">
      <PageHeader titleKey="acpManage.title" descriptionKey="acpManage.subtitle" />
      <div className="col-span-12">
        <ServiceStatusCard />
      </div>
      <div className="col-span-12 lg:col-span-6">
        <WorkspaceManageCard />
      </div>
      <div className="col-span-12 lg:col-span-6">
        <SessionsCard />
      </div>
      <span className="sr-only">{t("acpManage.subtitle")}</span>
    </div>
  );
}