// ACS1 redirects 验收（任务书第 4 条）：三条深链兜底逐条机械核验。
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { agentRedirectTarget } from "./agent-redirect";

describe("agentRedirectTarget（ACS1 深链兜底）", () => {
  it("第一条：/chat?agent=X -> /agent?endpoint=X", () => {
    expect(agentRedirectTarget("?agent=ep-1")).toBe("/agent?endpoint=ep-1");
  });

  it("第二条：/chat?kind=agent -> /agent", () => {
    expect(agentRedirectTarget("?kind=agent")).toBe("/agent");
  });

  it("非 agent 深链不改道（/chat 行为不变）", () => {
    expect(agentRedirectTarget("")).toBeNull();
    expect(agentRedirectTarget("?peer=p1")).toBeNull();
    expect(agentRedirectTarget("?kind=group")).toBeNull();
    expect(agentRedirectTarget("?kind=a2a")).toBeNull();
  });

  it("agent= 优先级高于 kind=agent，其余参数透传", () => {
    expect(agentRedirectTarget("?agent=ep-1&kind=agent")).toBe(
      "/agent?kind=agent&endpoint=ep-1",
    );
    expect(agentRedirectTarget("?kind=agent&compose=hi")).toBe("/agent?compose=hi");
  });
});

describe("第三条：/acp redirect 目标改 /agent", () => {
  it("App.tsx 的 acp 路由重定向到独立会话页", () => {
    const source = readFileSync(join(process.cwd(), "src", "App.tsx"), "utf8");
    const line = source.split("\n").find((l) => l.includes('path="acp"'));
    expect(line, "App.tsx 缺 acp 重定向路由").toBeTruthy();
    expect(line).toContain('to="/agent"');
    expect(line).not.toContain('to="/chat');
  });
});
