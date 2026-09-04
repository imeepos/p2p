// 中央菜单登记表（append-only）：新增视图在此注册路由/标题 i18n key/图标，
// 注册变更压独立小提交，不得混入 feature 提交。
import {
  Activity,
  Bot,
  LayoutDashboard,
  MessageCircle,
  Network,
  Radar,
  Settings,
  Stethoscope,
  Waypoints,
  type LucideIcon,
} from "lucide-react";

import type { I18nKey } from "@/i18n/types";

export interface MenuEntry {
  path: string;
  titleKey: I18nKey;
  icon: LucideIcon;
}

export const MENU_ENTRIES: readonly MenuEntry[] = [
  { path: "/", titleKey: "dashboard.title", icon: LayoutDashboard },
  { path: "/peers", titleKey: "peers.title", icon: Network },
  { path: "/discovery", titleKey: "discovery.title", icon: Radar },
  { path: "/relay", titleKey: "relay.title", icon: Waypoints },
  { path: "/chat", titleKey: "chat.title", icon: MessageCircle },
  { path: "/acp", titleKey: "acp.title", icon: Bot },
  { path: "/events", titleKey: "events.title", icon: Activity },
  { path: "/settings", titleKey: "settings.title", icon: Settings },
  { path: "/diagnostics", titleKey: "diagnostics.title", icon: Stethoscope },
];
