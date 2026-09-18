import { Fragment } from "react";
import {
  InfoIcon,
  NetworkIcon,
  RadarIcon,
  ServerIcon,
  Settings2Icon,
  UserRoundIcon,
  WrenchIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { useUpdateStore } from "@/stores/update-store";
import { cn } from "@/lib/utils";

// 分节导航配置：id 与右侧内容分节一一对应（settings-view 的显隐容器）。
// 「运维」项前渲染分组分隔线：上组为配置分区，下组为系统分区（2026-09-18 IA 重组）。
const NAV_ITEMS = [
  { id: "account", labelKey: "settings.nav.account", Icon: UserRoundIcon },
  { id: "general", labelKey: "settings.nav.general", Icon: Settings2Icon },
  { id: "network", labelKey: "settings.nav.network", Icon: NetworkIcon },
  { id: "services", labelKey: "settings.nav.services", Icon: ServerIcon },
  { id: "remoteAccess", labelKey: "settings.nav.remoteAccess", Icon: RadarIcon },
  { id: "ops", labelKey: "settings.nav.ops", Icon: WrenchIcon },
  { id: "about", labelKey: "settings.nav.about", Icon: InfoIcon },
] as const;

export type SettingsSectionId = (typeof NAV_ITEMS)[number]["id"];

interface SettingsNavProps {
  active: SettingsSectionId;
  onSelect: (id: SettingsSectionId) => void;
}

// 微信设置式左栏：选中项绿色圆角块（主题 primary 即 #07c160），
// 有可用更新时「关于」项带红点（对应微信菜单红点语义）。
export function SettingsNav({ active, onSelect }: SettingsNavProps) {
  const { t } = useTranslation();
  const updateAvailable = useUpdateStore(
    (s) => s.status === "available" && s.result != null,
  );

  return (
    <aside
      aria-label={t("settings.title")}
      data-testid="settings-nav"
      className="bg-muted/40 ring-border flex w-48 shrink-0 flex-col gap-1 rounded-lg p-2 ring-1"
    >
      {NAV_ITEMS.map(({ id, labelKey, Icon }) => {
        const selected = id === active;
        return (
          <Fragment key={id}>
            {id === "ops" ? (
              <div
                role="presentation"
                className="bg-border mx-2 h-px"
                data-testid="settings-nav-divider"
              />
            ) : null}
            <button
              type="button"
              aria-current={selected ? "true" : undefined}
              data-testid={`settings-nav-${id}`}
              onClick={() => onSelect(id)}
              className={cn(
                "flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                selected
                  ? "bg-primary text-primary-foreground"
                  : "text-foreground/80 hover:bg-accent",
              )}
            >
              <Icon aria-hidden className="size-4 shrink-0" />
              <span className="flex-1 truncate text-left">{t(labelKey)}</span>
              {id === "about" && updateAvailable ? (
                <span aria-hidden className="size-2 shrink-0 rounded-full bg-destructive" />
              ) : null}
            </button>
          </Fragment>
        );
      })}
    </aside>
  );
}
