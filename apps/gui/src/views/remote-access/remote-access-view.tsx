import { PageHeader } from "@/components/page/page-header";

import { DshOpenCard } from "./dsh-open-card";
import { GenericTunnelCard } from "./generic-tunnel-card";
import { TunnelServeCard } from "./tunnel-serve-card";
import { TunnelTerminalBanner } from "./tunnel-terminal-banner";
import { TunnelStatusCard } from "./tunnel-status-card";
import { useTunnelPageModel } from "./use-tunnel-page-model";

// 远程访问页（W-T3）：双语义——把本机任意 http/ws 服务分享给指定节点
// （被访侧 serve，含对端说明与白名单表格）/ 访问对方节点服务（访侧 DSH
// + 通用入口，含好友选择与 ws 派生地址）。终态统一以页面横幅呈现。
export function RemoteAccessView() {
  const page = useTunnelPageModel();
  return (
    <>
      <PageHeader
        titleKey="remoteAccess.title"
        descriptionKey="remoteAccess.description"
      />
      {page.banner ? (
        <div className="col-span-12">
          <TunnelTerminalBanner
            banner={page.banner}
            onDismiss={page.dismissBanner}
          />
        </div>
      ) : null}
      <DshOpenCard
        phase={page.phase}
        onOpened={page.handleOpened}
        onError={page.handleError}
        onBanner={page.setBanner}
      />
      <GenericTunnelCard
        onOpened={page.handleOpened}
        onError={page.handleError}
        onBanner={page.setBanner}
      />
      <TunnelServeCard
        serve={page.status.serve}
        onServeUpdate={page.handleServeUpdate}
        onError={page.handleError}
        onBanner={page.setBanner}
      />
      <TunnelStatusCard status={page.status} openUrl={page.openUrl} />
    </>
  );
}
