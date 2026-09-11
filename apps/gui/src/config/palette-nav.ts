import {
  Activity,
  Bell,
  BookOpen,
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
// F22：/network/overview 标签与页头同源（network.overview.title），全局
// 统一叫「概览」，不再出现面板「仪表盘」/页头「网络概览」两名并存。
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
  // IMC3：消息中心（append-only 登记，与 rail 注册同步）
  { path: "/messages", labelKey: "messages.title", icon: Bell },
  // DOC2：协议文档页（append-only 登记，与 rail 注册同步）
  { path: "/docs", labelKey: "docs.title", icon: BookOpen },
  // F22：与页头 overview-view 同源 i18n key
  { path: "/network/overview", labelKey: "network.overview.title", icon: LayoutDashboard },
  { path: "/network/peers", labelKey: "peers.title", icon: Network },
  { path: "/network/discovery", labelKey: "discovery.title", icon: Radar },
  { path: "/network/relay", labelKey: "relay.title", icon: Waypoints },
  { path: "/network/events", labelKey: "events.title", icon: Activity },
  { path: "/network/diagnostics", labelKey: "diagnostics.title", icon: Stethoscope },
  // W-T3：远程访问页（append-only 登记，与路由同步，rail 不动）
  { path: "/remote-access", labelKey: "remoteAccess.title", icon: Radar },
  { path: "/contacts#friends", labelKey: "contacts.section.friends", icon: UsersRound },
  { path: "/contacts#groups", labelKey: "contacts.section.groups", icon: UsersRound },
  { path: "/contacts#agents", labelKey: "contacts.section.agents", icon: Bot },
];
