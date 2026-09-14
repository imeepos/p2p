// 本地 ACP 卡测试：工作区清单呈现 + 分享入口（信息架构本地区主体）。
import { act, fireEvent, render, screen } from "@testing-library/react";
import { Toaster, toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { LocalAcpCard } = await import("./local-acp-card");
const { useAcpStore } = await import("../acp-store");
const { resetFixtures } = await import("../acp-view-test-utils");
const { resetToastDedupForTest } = await import("@/components/feedback/toast");
const { mockBackend } = await import("@/lib/mock-ipc");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
  resetToastDedupForTest();
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

function workspaceFetch(name: string) {
  return vi.fn(async (url: string) => {
    if (String(url).endsWith("/workspaces")) {
      return {
        ok: true,
        status: 200,
        json: async () => ({ workspaces: [{ id: "ws1", name, dir: "/home/me/p2p" }] }),
      };
    }
    return { ok: true, status: 200, json: async () => ({}) };
  });
}

// AF2 接线：刷新按钮补 pending 微反馈与成功 toast（列表取数失败静默吞错是
// share-admin-client.listWorkspaces 契约现状，错误 toast 见 AF2 汇报顾虑）。
describe("LocalAcpCard 刷新反馈（AF2）", () => {
  it("点击刷新即 pending（disabled+aria-busy），完成后 toast「已刷新」", async () => {
    vi.stubGlobal("fetch", workspaceFetch("p2p"));
    render(
      <>
        <LocalAcpCard />
        <Toaster position="bottom-right" />
      </>,
    );
    expect(await screen.findByText("p2p")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-local-refresh"));
    const btn = screen.getByTestId("acp-local-refresh");
    expect(btn.hasAttribute("disabled")).toBe(true);
    expect(btn.getAttribute("aria-busy")).toBe("true");
    await screen.findByText("已刷新");
    act(() => {
      toast.dismiss();
    });
  });

  it("刷新后以最新清单替换旧行", async () => {
    let round = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (String(url).endsWith("/workspaces")) {
          round += 1;
          return workspaceFetch(round === 1 ? "p2p" : "p2p-renamed")(url);
        }
        return { ok: true, status: 200, json: async () => ({}) };
      }),
    );
    render(<LocalAcpCard />);
    expect(await screen.findByText("p2p")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-local-refresh"));
    expect(await screen.findByText("p2p-renamed")).toBeTruthy();
  });
});
