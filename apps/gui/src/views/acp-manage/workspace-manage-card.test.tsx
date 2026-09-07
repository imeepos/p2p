// 工作区管理卡测试：清单渲染、新增 POST 契约与刷新、删除确认后 DELETE、
// 错误词法码人话映射（duplicate-id → 「该 ID 已存在」）。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { WorkspaceManageCard } = await import("./workspace-manage-card");
const { ConfirmProvider } = await import("@/components/feedback/confirm-provider");
const { useAcpStore } = await import("@/acp/acp-store");
const { resetFixtures } = await import("@/acp/acp-view-test-utils");
await import("@/i18n");

function stubFetch(handler: (url: string, init?: RequestInit) => { status: number; body: unknown }) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const { status, body } = handler(url, init);
    return { ok: status < 400, status, json: async () => body };
  });
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

describe("WorkspaceManageCard 渲染与增删", () => {
  it("清单渲染：id 与目录如实显示", async () => {
    stubFetch(() => ({
      status: 200,
      body: { workspaces: [{ id: "docs", name: "Docs", dir: "/tmp/docs" }] },
    }));
    render(
      <ConfirmProvider>
        <WorkspaceManageCard />
      </ConfirmProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("acp-ws-row-docs")).toBeTruthy();
    });
    expect(screen.getByTestId("acp-ws-row-docs").textContent).toContain("/tmp/docs");
  });

  it("新增：表单提交 POST /workspaces 后刷新清单", async () => {
    const fetchMock = stubFetch((_url, init) => {
      if (init?.method === "POST") {
        return { status: 200, body: { workspace: { id: "demo", name: "Demo", dir: "/tmp/demo" } } };
      }
      return { status: 200, body: { workspaces: [] } };
    });
    render(
      <ConfirmProvider>
        <WorkspaceManageCard />
      </ConfirmProvider>,
    );
    fireEvent.change(screen.getByLabelText("ID"), { target: { value: "demo" } });
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Demo" } });
    fireEvent.change(screen.getByLabelText("目录（本机绝对路径）"), { target: { value: "/tmp/demo" } });
    fireEvent.click(screen.getByTestId("acp-ws-add-submit"));
    await waitFor(() => {
      const posts = fetchMock.mock.calls.filter(
        ([, init]) => (init as RequestInit | undefined)?.method === "POST",
      );
      expect(posts.length).toBe(1);
      expect(JSON.stringify(posts[0][1]?.body)).toContain("demo");
    });
  });

  it("错误映射：duplicate-id 显示人话文案而非裸状态码", async () => {
    stubFetch((_url, init) => {
      if (init?.method === "POST") {
        return { status: 409, body: { error: "duplicate-id" } };
      }
      return { status: 200, body: { workspaces: [] } };
    });
    render(
      <ConfirmProvider>
        <WorkspaceManageCard />
      </ConfirmProvider>,
    );
    fireEvent.change(screen.getByLabelText("ID"), { target: { value: "docs" } });
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "Docs" } });
    fireEvent.change(screen.getByLabelText("目录（本机绝对路径）"), { target: { value: "/tmp/docs" } });
    fireEvent.click(screen.getByTestId("acp-ws-add-submit"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-ws-add-error").textContent).toContain("已存在");
    });
  });

  it("删除：确认弹框通过后 DELETE /workspaces/{id}", async () => {
    const fetchMock = stubFetch((_url, init) =>
      init?.method === "DELETE"
        ? { status: 200, body: { deleted: true } }
        : { status: 200, body: { workspaces: [{ id: "docs", name: "Docs", dir: "/tmp/docs" }] } },
    );
    render(
      <ConfirmProvider>
        <WorkspaceManageCard />
      </ConfirmProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("acp-ws-row-docs")).toBeTruthy();
    });
    fireEvent.click(screen.getByTestId("acp-ws-remove-docs"));
    fireEvent.click(await screen.findByRole("button", { name: "删除" }));
    await waitFor(() => {
      const deletes = fetchMock.mock.calls.filter(
        ([, init]) => (init as RequestInit | undefined)?.method === "DELETE",
      );
      expect(deletes.length).toBe(1);
      expect(String(deletes[0][0])).toContain("/workspaces/docs");
    });
  });
});