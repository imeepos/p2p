// 中央菜单登记表（append-only）：rail 一级入口在此注册路由/标题 i18n key/图标，
// 注册变更压独立小提交，不得混入 feature 提交。
// 外壳重设计后 rail 恰 4 个一级入口（docs/design/app-shell-redesign.md 1.1），
// 注册序即快捷键 Cmd/Ctrl+1..4 映射序；末项（设置）由 rail 布局沉底。
// 子页/tab 不进本表：命令面板持有独立全量注册表（5.2 解耦约束）。
import {
  MessageCircle,
  Network,
  Settings,
  UsersRound,
  type LucideIcon,
} from "lucide-react";

import type { I18nKey } from "@/i18n/types";

export interface MenuEntry {
  path: string;
  titleKey: I18nKey;
  icon: LucideIcon;
}

export const MENU_ENTRIES: readonly MenuEntry[] = [
  { path: "/chat", titleKey: "chat.title", icon: MessageCircle },
  { path: "/contacts", titleKey: "contacts.title", icon: UsersRound },
  { path: "/network", titleKey: "network.title", icon: Network },
  { path: "/settings", titleKey: "settings.title", icon: Settings },
];
