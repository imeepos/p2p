import { useTranslation } from "react-i18next";
import {
  BellIcon,
  BookOpenIcon,
  NetworkIcon,
  ServerIcon,
} from "lucide-react";

import type { I18nKey } from "@/i18n/types";
import { selectPendingInviteBadgeCount } from "@/stores/chat-group-invite-slice";
import { useChatStore } from "@/stores/chat-store";
import { EntryCard } from "./entry-card";

interface OpsEntry {
  path: string;
  titleKey: I18nKey;
  descKey: I18nKey;
  icon: typeof NetworkIcon;
}

// 2026-09-18 一级入口口径拍板：配置辅助不进 rail，收敛为设置页运维区入口行；
// 路由与 ⌘K 命令面板全量保留，此处只是可达性入口。远程访问入口归位到
// 「远程访问」分区（消除左栏双「远程访问」）。
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
// pending 之和），rail 收敛后邀请可见性的承接点。
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
        <EntryCard
          key={entry.path}
          path={entry.path}
          titleKey={entry.titleKey}
          descKey={entry.descKey}
          icon={entry.icon}
          badge={
            entry.path === "/messages"
              ? { count: pendingInvites, ariaKey: "messages.badgeAria" }
              : undefined
          }
        />
      ))}
    </>
  );
}
