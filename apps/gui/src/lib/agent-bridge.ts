// Agent 页面操作入口（G-H）：dev 构建或 URL hash 带 agent=1 时暴露 window.__P2P_AGENT__。
// 供 scripts/gui-agent.mjs 经 CDP eval 调用（导航/错误缓冲/模式探针）；页面不新增视觉元素。
import type { FrontendErrorEntry } from "./error-report";

import { getRecentErrors } from "./error-report";
import { useMockIpc } from "./ipc";

export interface AgentBridge {
  mode: "mock" | "tauri";
  recentErrors: () => FrontendErrorEntry[];
  navigateTo: (path: string) => void;
  ping: () => "pong";
}

declare global {
  interface Window {
    __P2P_AGENT__?: AgentBridge;
  }
}

export function installAgentBridge(): void {
  if (!agentAllowed() || window.__P2P_AGENT__) return;
  window.__P2P_AGENT__ = {
    mode: useMockIpc ? "mock" : "tauri",
    recentErrors: () => [...getRecentErrors()],
    navigateTo: (path: string) => {
      window.location.hash = "#" + path;
    },
    ping: () => "pong",
  };
}

function agentAllowed(): boolean {
  if (import.meta.env.DEV) return true;
  return window.location.hash.includes("agent=1");
}