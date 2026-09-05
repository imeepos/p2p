import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { MENU_ENTRIES } from "@/config/menu.def";
import { cn } from "@/lib/utils";

// 窄图标栏（docs/design/app-shell-redesign.md 1.1）：常驻 w-14，仅图标 +
// tooltip + 选中态高亮，不再提供折叠形态；注册序末项（设置）沉底。
// 顶栏与底部状态栏不在 rail 职责内，由 AppLayout 组装。
function RailLink({ path, titleKey, icon: Icon }: (typeof MENU_ENTRIES)[number]) {
  const { t } = useTranslation();
  const label = t(titleKey);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <NavLink
          to={path}
          aria-label={label}
          className={({ isActive }) =>
            cn(
              "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-ring/50 flex size-9 items-center justify-center rounded-md focus-visible:ring-[3px] focus-visible:outline-none",
              isActive && "bg-sidebar-accent text-sidebar-accent-foreground font-semibold",
            )
          }
        >
          <Icon className="size-4 shrink-0" aria-hidden />
        </NavLink>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

export function IconRail() {
  const top = MENU_ENTRIES.slice(0, -1);
  const bottom = MENU_ENTRIES[MENU_ENTRIES.length - 1];
  return (
    <aside className="bg-sidebar text-sidebar-foreground flex h-full w-14 flex-col border-r">
      <nav className="flex flex-1 flex-col items-center gap-1 p-2">
        {top.map((entry) => (
          <RailLink key={entry.path} {...entry} />
        ))}
        {/* 弹性空隙：高频入口（聊天/通讯录/网络）居上，低频设置沉底 */}
        <div className="flex-1" />
        {bottom ? <RailLink {...bottom} /> : null}
      </nav>
    </aside>
  );
}
