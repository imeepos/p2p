import { CircleUserRoundIcon } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useNodeStore } from "@/stores/node-store";
import { useProfileStore } from "@/stores/profile-store";

// 侧边栏身份徽标：头像 + 节点名称（未设置回退占位文案），点击进设置页编辑。
// 资料加载失败不阻塞导航，回退默认展示（信号由 store 留）。
export function IdentityBadge({ collapsed }: { collapsed: boolean }) {
  const { t } = useTranslation();
  const profile = useProfileStore((s) => s.profile);
  const loaded = useProfileStore((s) => s.loaded);
  const load = useProfileStore((s) => s.load);
  const peerId = useNodeStore((s) => s.status?.peerId ?? null);

  useEffect(() => {
    if (!loaded) {
      load().catch(() => {});
    }
  }, [loaded, load]);

  const name = profile.name.trim() || t("settings.profile.unnamed");
  const shortId = peerId ? peerId.slice(0, 8) + "…" + peerId.slice(-4) : null;
  const avatar = profile.avatar ? (
    <img
      src={profile.avatar}
      alt={t("settings.profile.avatarAlt")}
      className="size-7 shrink-0 rounded-full object-cover"
    />
  ) : (
    <CircleUserRoundIcon aria-hidden className="text-muted-foreground size-7 shrink-0" />
  );

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Link
            to="/settings"
            aria-label={t("settings.profile.editProfile")}
            className="hover:bg-sidebar-accent flex items-center justify-center rounded-md py-1"
          >
            {avatar}
          </Link>
        </TooltipTrigger>
        <TooltipContent side="right">{name}</TooltipContent>
      </Tooltip>
    );
  }

  return (
    <Link
      to="/settings"
      title={t("settings.profile.editProfile")}
      className="hover:bg-sidebar-accent flex items-center gap-2 rounded-md px-2 py-1.5"
    >
      {avatar}
      <span className="min-w-0 flex-1 leading-tight">
        <span className="block truncate text-sm font-medium">{name}</span>
        {shortId ? (
          <span className="text-muted-foreground block truncate font-mono text-[10px]">
            {shortId}
          </span>
        ) : null}
      </span>
    </Link>
  );
}
