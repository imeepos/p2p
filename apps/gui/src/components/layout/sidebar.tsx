import { PanelLeftCloseIcon, PanelLeftOpenIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { MENU_ENTRIES } from "@/config/menu.def";
import { cn } from "@/lib/utils";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { t } = useTranslation();

  return (
    <aside
      className={cn(
        "bg-sidebar text-sidebar-foreground flex h-full flex-col border-r transition-[width] motion-reduce:transition-none",
        collapsed ? "w-14" : "w-60",
      )}
    >
      <nav className="flex flex-1 flex-col gap-1 p-2">
        {MENU_ENTRIES.map((entry) => (
          <NavLink
            key={entry.path}
            to={entry.path}
            end={entry.path === "/"}
            title={collapsed ? t(entry.titleKey) : undefined}
            className={({ isActive }) =>
              cn(
                "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground flex h-9 items-center gap-3 rounded-md px-3 text-sm font-medium",
                isActive &&
                  "bg-sidebar-accent text-sidebar-accent-foreground font-semibold",
                collapsed && "justify-center px-0",
              )
            }
          >
            <entry.icon className="size-4 shrink-0" aria-hidden />
            {!collapsed && <span className="truncate">{t(entry.titleKey)}</span>}
          </NavLink>
        ))}
      </nav>
      <div className="p-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="w-full"
              onClick={onToggle}
              aria-label={t(
                collapsed ? "common.actions.expand" : "common.actions.collapse",
              )}
            >
              {collapsed ? <PanelLeftOpenIcon /> : <PanelLeftCloseIcon />}
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            {t(collapsed ? "common.actions.expand" : "common.actions.collapse")}
          </TooltipContent>
        </Tooltip>
      </div>
    </aside>
  );
}
