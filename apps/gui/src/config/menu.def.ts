// 中央菜单登记表（append-only）：rail 一级入口在此注册路由/标题 i18n key/图标，
// 注册变更压独立小提交，不得混入 feature 提交。
// 外壳重设计后 rail 为 4 个一级入口（docs/design/app-shell-redesign.md 1.1），
// F15：消息中心升 rail 常驻（顶栏铃铛保留为快捷方式）；R2-13：LLM 共享升 rail。
// 2026-09-09：协议文档(/docs)移出 rail 常驻，页面与路由保留，
// 经 ⌘K 命令面板/直链可达（palette-nav.ts 仍全量注册）。
// 2026-09-18 一级入口口径拍板：rail 只放高频核心功能；网络(6 tab)/消息中心/
// 远程访问/ACP 管理/协议文档归「配置辅助」，入口收敛进设置页运维区
// （views/settings/ops-section.tsx），路由与 ⌘K 全量保留，rail 收敛为 6 项。
// 注册序即快捷键 Cmd/Ctrl+1..6 映射序；末项（设置）由 rail 布局沉底。
// 子页/tab 不进本表：命令面板持有独立全量注册表（5.2 解耦约束）。
import {
  Bot,
  MessageCircle,
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
  // ACS1（2026-09-14 用户拍板）：agent 独立会话页升 rail 一级入口，序插 /chat 后
  // （注册序即快捷键 Cmd/Ctrl+2；rail 上限 9 未被突破，后续项顺延不越界）
  { path: "/agent", titleKey: "agentChat.title", icon: Bot },
  { path: "/contacts", titleKey: "contacts.title", icon: UsersRound },
  // R2-13：LLM 共享升 rail（此前仅命令面板+设置入口卡可达）
  { path: "/llm-share", titleKey: "llmShare.title", icon: Share2 },
  // A2A3：智能体升 rail（a2a-over-p2p-design §8.1/拍板 Q7：llm-share 后、
  // settings 前；Sparkles；注册序即快捷键 Cmd/Ctrl+5，上限 9 吻合）
  { path: "/agents", titleKey: "agents.title", icon: Sparkles },
  { path: "/settings", titleKey: "settings.title", icon: Settings },
];
