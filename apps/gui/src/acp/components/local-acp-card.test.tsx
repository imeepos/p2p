// 本地 ACP 卡测试：工作区清单呈现 + 分享入口（信息架构本地区主体）。
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { LocalAcpCard } = await import("./local-acp-card");
const { useAcpStore } = await import("../acp-store");
const { resetFixtures } = await import("../acp-view-test-utils");
const { mockBackend } = await import("@/lib/mock-ipc");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
  useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p" } });
  vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue({
    adminUrl: "http://127.0.0.1:8123",
    token: "auto-tok",
    peer: "peerLOCAL",
    agentName: "home-agent",
    writtenAtUnix: 1_725_700_000,
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("LocalAcpCard 本地工作区", () => {
  it("列出 admin 返回的工作区（名称+目录），每行带分享入口", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (String(url).endsWith("/workspaces")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            workspaces: [{ id: "ws1", name: "p2p", dir: "/home/me/p2p" }],
          }),
        };
      }
      return { ok: true, status: 200, json: async () => ({}) };
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<LocalAcpCard />);
    expect(await screen.findByText("p2p")).toBeTruthy();
    expect(screen.getByText("/home/me/p2p")).toBeTruthy();
    expect(screen.getByTestId("acp-local-share-ws1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-local-share-ws1"));
    expect(await screen.findByTestId("acp-share-dialog")).toBeTruthy();
  });

  it("旧 agent 无 workspaces 端点：显示空态引导", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: false, status: 404, json: async () => ({ error: "not-found" }) })),
    );
    render(<LocalAcpCard />);
    expect(await screen.findByTestId("acp-local-workspace-empty")).toBeTruthy();
  });
});
