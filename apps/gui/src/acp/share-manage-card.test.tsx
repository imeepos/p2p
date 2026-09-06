// owner 分享管理卡测试：台账渲染与五态徽章、撤销确认后 DELETE 契约、
// 空态/未登记 admin 端点引导/加载失败显式报错。
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareManageCard } = await import("./components/share-manage-card");
const { ConfirmProvider } = await import("@/components/feedback/confirm-provider");
const { useAcpStore } = await import("./acp-store");
const { resetFixtures } = await import("./acp-view-test-utils");
await import("@/i18n");

function entryOf(id: string, patch: Record<string, unknown>) {
  return {
    share_id: id,
    scope: "sandbox",
    allow_mcp: [],
    max_activations: 1,
    activations: 0,
    expires_at_unix: Math.floor(Date.now() / 1000) + 3_600,
    revoked: false,
    note: "note-" + id,
    created_at: "2026-09-06T00:00:00Z",
    bound_peer: null,
    ...patch,
  };
}

function stubList(shares: unknown[]) {
  const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => ({
    ok: true,
    status: 200,
    json: async () =>
      init?.method === "DELETE" ? {} : { shares },
  }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

beforeEach(() => {
  resetFixtures();
  useAcpStore.setState({ directory: [] });
  useAcpStore.getState().setDraft({
    adminUrl: "http://127.0.0.1:9910",
    adminToken: "admin-tok",
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ShareManageCard 渲染与撤销", () => {
  it("台账五态徽章如实渲染：有效/已绑定/已用尽/已过期/已撤销", async () => {
    stubList([
      entryOf("s-active", {}),
      entryOf("s-bound", { max_activations: 3, bound_peer: "peerABC" }),
      entryOf("s-exhausted", { activations: 1 }),
      entryOf("s-expired", { expires_at_unix: 1 }),
      entryOf("s-revoked", { revoked: true }),
    ]);
    render(
      <ConfirmProvider>
        <ShareManageCard />
      </ConfirmProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("acp-share-row-s-active")).toBeTruthy();
    });
    const statusOf = (id: string) => screen.getByTestId("acp-share-status-" + id).textContent;
    expect(statusOf("s-active")).toContain("有效");
    expect(statusOf("s-bound")).toContain("已绑定");
    expect(statusOf("s-exhausted")).toContain("已用尽");
    expect(statusOf("s-expired")).toContain("已过期");
    expect(statusOf("s-revoked")).toContain("已撤销");
    expect(screen.queryByTestId("acp-share-revoke-s-revoked")).toBeNull();
  });

  it("撤销：确认弹框通过后 DELETE /shares/{id} 并刷新清单", async () => {
    const fetchMock = stubList([entryOf("sid-1", {})]);
    render(
      <ConfirmProvider>
        <ShareManageCard />
      </ConfirmProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("acp-share-row-sid-1")).toBeTruthy();
    });
    fireEvent.click(screen.getByTestId("acp-share-revoke-sid-1"));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("撤销该分享？");
    fireEvent.click(within(dialog).getByText("撤销"));
    await waitFor(() => {
      const deleteCall = fetchMock.mock.calls.find(
        ([, init]) => (init as RequestInit | undefined)?.method === "DELETE",
      );
      expect(deleteCall).toBeTruthy();
      expect(String(deleteCall![0])).toBe("http://127.0.0.1:9910/shares/sid-1");
    });
  });

  it("空台账显空态引导；admin 端点未登记显登记引导且不发请求", async () => {
    const fetchMock = stubList([]);
    render(
      <ConfirmProvider>
        <ShareManageCard />
      </ConfirmProvider>,
    );
    expect(await screen.findByTestId("acp-share-manage-empty")).toBeTruthy();
    expect(String(fetchMock.mock.calls[0][0])).toBe("http://127.0.0.1:9910/shares");

    useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p" } });
    render(
      <ConfirmProvider>
        <ShareManageCard />
      </ConfirmProvider>,
    );
    expect(await screen.findAllByTestId("acp-share-manage-need-admin")).toHaveLength(2);
  });

  it("加载失败显式报错并提供重载入口", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("admin down");
      }),
    );
    render(
      <ConfirmProvider>
        <ShareManageCard />
      </ConfirmProvider>,
    );
    expect(await screen.findByTestId("acp-share-manage-error")).toBeTruthy();
    expect(screen.getByTestId("acp-share-manage-reload")).toBeTruthy();
  });
});
