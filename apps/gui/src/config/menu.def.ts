// 中央菜单登记表（append-only）：rail 一级入口在此注册路由/标题 i18n key/图标，
// 注册变更压独立小提交，不得混入 feature 提交。
// 外壳重设计后 rail 为 4 个一级入口（docs/design/app-shell-redesign.md 1.1），
// F15 升为 6：消息中心/协议文档常驻可发现（⌘K 与顶栏铃铛保留为快捷方式），
// R2-13 升为 7：LLM 共享常驻可达；注册序即快捷键 Cmd/Ctrl+1..7 映射序；
// 末项（设置）由 rail 布局沉底。
// 子页/tab 不进本表：命令面板持有独立全量注册表（5.2 解耦约束）。
import {
  Bell,
  BookOpen,
  MessageCircle,
  Network,
  Settings,
  Share2,
  Sparkles,
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
  // F15：消息中心/协议文档升 rail 常驻（append-only 注册）
  { path: "/messages", titleKey: "messages.title", icon: Bell },
  { path: "/docs", titleKey: "docs.title", icon: BookOpen },
  // R2-13：LLM 共享升 rail（此前仅命令面板+设置入口卡可达）
  { path: "/llm-share", titleKey: "llmShare.title", icon: Share2 },
  // A2A3：智能体升 rail（a2a-over-p2p-design §8.1/拍板 Q7：llm-share 后、
  // settings 前；Sparkles；注册序即快捷键 Cmd/Ctrl+8，上限 9 吻合）
  { path: "/agents", titleKey: "agents.title", icon: Sparkles },
  { path: "/settings", titleKey: "settings.title", icon: Settings },
];
