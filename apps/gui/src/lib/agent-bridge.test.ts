import { describe, expect, it } from "vitest";

import { installAgentBridge } from "./agent-bridge";

describe("agent-bridge", () => {
  it("dev 环境暴露 window.__P2P_AGENT__ 操作面", () => {
    installAgentBridge();
    const bridge = window.__P2P_AGENT__;
    expect(bridge).toBeDefined();
    expect(bridge?.ping()).toBe("pong");
    expect(typeof bridge?.recentErrors).toBe("function");
    expect(["mock", "tauri"]).toContain(bridge?.mode);
  });

  it("navigateTo 经 hash 路由跳转", () => {
    installAgentBridge();
    window.__P2P_AGENT__?.navigateTo("/peers");
    expect(window.location.hash).toBe("#/peers");
  });
});