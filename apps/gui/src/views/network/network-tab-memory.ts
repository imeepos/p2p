import type { I18nKey } from "@/i18n/types";

// tab 局部登记数组（append-only 约束，docs/design/app-shell-redesign.md 4.1）：
// 子路由即 tab，登记在本文件不进中央 menu.def；path 为 /network 下子段。
export const NETWORK_TABS = [
  { id: "overview", path: "overview", labelKey: "network.tabs.overview" },
  { id: "peers", path: "peers", labelKey: "network.tabs.peers" },
  { id: "discovery", path: "discovery", labelKey: "network.tabs.discovery" },
  { id: "relay", path: "relay", labelKey: "network.tabs.relay" },
  { id: "events", path: "events", labelKey: "network.tabs.events" },
  {
    id: "diagnostics",
    path: "diagnostics",
    labelKey: "network.tabs.diagnostics",
  },
] as const satisfies readonly {
  id: string;
  path: string;
  labelKey: I18nKey;
}[];

export type NetworkTabId = (typeof NETWORK_TABS)[number]["id"];

// /network/<segment> 段 -> tab id；非 tab 路径返回 null（调用方不记忆）。
export function tabIdForPath(pathname: string): NetworkTabId | null {
  const segment = pathname.replace(/^\/network\/?/, "").split("/")[0];
  return NETWORK_TABS.find((tab) => tab.path === segment)?.id ?? null;
}

// 会话级 tab 记忆：模块作用域即窗口会话生命周期，不落盘、重启归零。
// /network 无子路由时落回最后停留 tab；全新会话默认落 overview。
const DEFAULT_TAB: NetworkTabId = "overview";
let lastTab: NetworkTabId | null = null;

export function rememberNetworkTab(id: NetworkTabId): void {
  lastTab = id;
}

export function recallNetworkTab(): NetworkTabId {
  return lastTab ?? DEFAULT_TAB;
}

// 测试隔离用：清空会话记忆回到全新会话语义。
export function resetNetworkTabMemory(): void {
  lastTab = null;
}
