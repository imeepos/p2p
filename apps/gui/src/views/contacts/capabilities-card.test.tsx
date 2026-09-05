// 能力卡视觉降级测试（P2 页面打磨）：支持 = success；未声明/不支持 = 灰化
// + 说明 title，三者不混淆。直接播种 store 渲染卡片。
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { InitializeResult } from "@/acp/protocol";
import { CapabilitiesCard } from "./capabilities-card";

await import("@/i18n");

function capsResult(agentCapabilities: InitializeResult["agentCapabilities"]): InitializeResult {
  return { protocolVersion: 1, agentInfo: { name: "mock-agent" }, agentCapabilities };
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
});

describe("CapabilitiesCard 能力位降级", () => {
  it("支持 = success 徽章，行不灰化无说明 title", () => {
    useAcpStore.setState({ phase: "online", capabilities: capsResult({ loadSession: true }) });
    render(<CapabilitiesCard />);
    const row = screen.getByTestId("acp-cap-loadSession");
    expect(row.textContent).toContain("支持");
    expect(row.className).not.toContain("opacity-80");
    expect(screen.queryByTitle("agent 已声明不支持此能力")).toBeNull();
  });

  it("不支持 = 灰化行 + 说明 title，不与支持混淆", () => {
    useAcpStore.setState({
      phase: "online",
      capabilities: capsResult({ loadSession: false, promptCapabilities: {} }),
    });
    render(<CapabilitiesCard />);
    const row = screen.getByTestId("acp-cap-loadSession");
    expect(row.textContent).toContain("不支持");
    expect(row.className).toContain("opacity-80");
    expect(row.querySelector("span")?.className).toContain("text-muted-foreground");
    expect(screen.getByTitle("agent 已声明不支持此能力")).toBeTruthy();
  });

  it("未声明 = 灰化行 + 未声明说明 title", () => {
    useAcpStore.setState({ phase: "online", capabilities: capsResult({ promptCapabilities: {} }) });
    render(<CapabilitiesCard />);
    const row = screen.getByTestId("acp-cap-loadSession");
    expect(row.textContent).toContain("未声明");
    expect(row.className).toContain("opacity-80");
    expect(row.querySelector("span[title]")?.getAttribute("title")).toBe(
      "agent 未在 initialize 中声明此能力",
    );
  });

  it("image/audio 能力位补齐展示，走统一降级样式（P2-ADD 需求10）", () => {
    useAcpStore.setState({
      phase: "online",
      capabilities: capsResult({
        loadSession: true,
        promptCapabilities: { embeddedContext: false, image: true, audio: false },
      }),
    });
    render(<CapabilitiesCard />);
    const image = screen.getByTestId("acp-cap-image");
    expect(image.textContent).toContain("图像输入");
    expect(image.textContent).toContain("支持");
    expect(image.className).not.toContain("opacity-80");
    const audio = screen.getByTestId("acp-cap-audio");
    expect(audio.textContent).toContain("音频输入");
    expect(audio.textContent).toContain("不支持");
    expect(audio.className).toContain("opacity-80");
    const embedded = screen.getByTestId("acp-cap-embeddedContext");
    expect(embedded.textContent).toContain("不支持");
    expect(embedded.className).toContain("opacity-80");
  });
});
