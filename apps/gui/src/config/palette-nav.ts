import {
  Activity,
  Bot,
  LayoutDashboard,
  MessageCircle,
  Network,
  Radar,
  Settings,
  Stethoscope,
  UsersRound,
  Waypoints,
  type LucideIcon,
} from "lucide-react";

import type { I18nKey } from "@/i18n/types";

// 命令面板导航注册表（docs/design/app-shell-redesign.md 5.2）：与
// rail/menu.def 解耦的独立全量清单——rail 收敛不得减少面板项。
// 13 项 = 4 rail 入口 + 6 网络 tab + 3 通讯录锚点，覆盖全部子页/tab；
// tab 标签复用旧页 title key（旧 key 随组件复用，不新开命名空间）。
export interface PaletteNavEntry {
  path: string;
  labelKey: I18nKey;
  icon: LucideIcon;
}

export const PALETTE_NAV_ENTRIES: readonly PaletteNavEntry[] = [
  { path: "/chat", labelKey: "chat.title", icon: MessageCircle },
  { path: "/contacts", labelKey: "contacts.title", icon: UsersRound },
  { path: "/network", labelKey: "network.title", icon: Network },
  { path: "/settings", labelKey: "settings.title", icon: Settings },
  { path: "/network/overview", labelKey: "dashboard.title", icon: LayoutDashboard },
  { path: "/network/peers", labelKey: "peers.title", icon: Network },
  { path: "/network/discovery", labelKey: "discovery.title", icon: Radar },
  { path: "/network/relay", labelKey: "relay.title", icon: Waypoints },
  { path: "/network/events", labelKey: "events.title", icon: Activity },
  { path: "/network/diagnostics", labelKey: "diagnostics.title", icon: Stethoscope },
  { path: "/contacts#friends", labelKey: "contacts.section.friends", icon: UsersRound },
  { path: "/contacts#groups", labelKey: "contacts.section.groups", icon: UsersRound },
  { path: "/contacts#agents", labelKey: "contacts.section.agents", icon: Bot },
];
