import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { BookOpenIcon, NetworkIcon, RadarIcon, ServerIcon, BellIcon, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { I18nKey } from "@/i18n/types";
import { selectPendingInviteBadgeCount } from "@/stores/chat-group-invite-slice";
import { useChatStore } from "@/stores/chat-store";
import { SettingsGroup, SettingsRow } from "./settings-row";

interface OpsEntry {
  path: string;
  titleKey: I18nKey;
  descKey: I18nKey;
  icon: LucideIcon;
}

// 2026-09-18 一级入口口径拍板：配置辅助（网络监控/消息中心/远程访问/
// ACP 管理/协议文档）不进 rail，统一收敛为设置页运维区入口行；
// 路由与 ⌘K 命令面板全量保留，此处只是可达性入口。
const OPS_ENTRIES: readonly OpsEntry[] = [
  {
    path: "/network",
    titleKey: "network.title",
    descKey: "settings.ops.networkDesc",
    icon: NetworkIcon,
  },
  {
    path: "/messages",
    titleKey: "messages.title",
    descKey: "messages.description",
    icon: BellIcon,
  },
  {
    path: "/remote-access",
    titleKey: "remoteAccess.title",
    descKey: "settings.ops.remoteAccessDesc",
    icon: RadarIcon,
  },
  {
    path: "/acp-manage",
    titleKey: "acpManage.title",
    descKey: "acpManage.subtitle",
    icon: ServerIcon,
  },
  {
    path: "/docs",
    titleKey: "docs.title",
    descKey: "docs.settings.entryDescription",
    icon: BookOpenIcon,
  },
];

// 消息中心行待处理邀请徽标：与原 rail F15 角标同源 selector（两类 in 向
// pending 之和），rail 收敛后邀请可见性的承接点。打开按钮文案复用
// settings.llmShare.entryAction 通用动作键（与 docs.settings.entryAction
// 同义，不新开第三份重复键）。
function OpsEntryRow({ entry, badge }: { entry: OpsEntry; badge?: number }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const Icon = entry.icon;
  return (
    <SettingsGroup
      title={t(entry.titleKey)}
      description={t(entry.descKey)}
    >
      <SettingsRow
        control={
          <div className="flex items-center gap-2">
            {badge != null && badge > 0 ? (
              <Badge
                variant="destructive"
                aria-label={t("messages.badgeAria", { count: badge })}
                data-testid={`ops-badge-${entry.path}`}
              >
                {badge}
              </Badge>
            ) : null}
            <Icon aria-hidden className="text-muted-foreground size-4" />
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => navigate(entry.path)}
            >
              {t("settings.llmShare.entryAction")}
            </Button>
          </div>
        }
      />
    </SettingsGroup>
  );
}

export function OpsSection() {
  const { t } = useTranslation();
  const pendingInvites = useChatStore(selectPendingInviteBadgeCount);
  return (
    <>
      <section className="flex flex-col">
        <h2 className="text-sm font-semibold">{t("settings.ops.title")}</h2>
        <p className="text-muted-foreground mt-0.5 text-xs leading-5">
          {t("settings.ops.hint")}
        </p>
      </section>
      {OPS_ENTRIES.map((entry) => (
        <OpsEntryRow
          key={entry.path}
          entry={entry}
          badge={entry.path === "/messages" ? pendingInvites : undefined}
        />
      ))}
    </>
  );
}
