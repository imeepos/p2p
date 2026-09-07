// 分享创建弹层测试：表单校验、POST /shares 提交契约（scope/ttl/次数/备注）、
// 链接展示与「发送到当前聊天」联动、admin 端点缺失引导。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareCreateDialog } = await import("./components/share-create-dialog");
const { useAcpStore } = await import("./acp-store");
const { pickOption, resetFixtures } = await import("./acp-view-test-utils");
const { mockBackend } = await import("@/lib/mock-ipc");
await import("@/i18n");

const TOKEN32 = "f".repeat(32);
const LINK = "dsh-acp-share://v1?peer=peerA&token=" + TOKEN32;

function stubCreatefetch(link: string) {
  const fetchMock = vi.fn(async () => ({
    ok: true,
    status: 200,
    json: async () => ({ share_id: "sid-1", token: TOKEN32, link, expires_at_unix: 3_000 }),
  }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

/** 只取带 body 的 POST 调用（GET /workspaces 等查询不带 body，不参与创建契约断言） */
function callsOf(fetchMock: ReturnType<typeof vi.fn>) {
  return fetchMock.mock.calls
    .filter(([, init]) => !!(init as RequestInit | undefined)?.body)
    .map(
      ([url, init]) =>
        [url, JSON.parse((init as RequestInit).body as string)] as [string, Record<string, unknown>],
    );
}

beforeEach(() => {
  resetFixtures();
  useAcpStore.getState().setDraft({
    adminUrl: "http://127.0.0.1:9910",
    adminToken: "admin-token",
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ShareCreateDialog 表单校验与提交契约", () => {
  it("激活次数非法：行内报错且不发请求", async () => {
    const fetchMock = stubCreatefetch(LINK);
    render(<ShareCreateDialog open onOpenChange={vi.fn()} />);
    fireEvent.change(screen.getByTestId("acp-share-activations"), { target: { value: "0" } });
    fireEvent.click(screen.getByTestId("acp-share-create"));
    expect(await screen.findByTestId("acp-share-error-activations")).toBeTruthy();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("默认档提交契约：sandbox/1h/1 次/备注裁剪；改档后请求体跟随", async () => {
    const fetchMock = stubCreatefetch(LINK);
    render(<ShareCreateDialog open onOpenChange={vi.fn()} />);
    fireEvent.change(screen.getByTestId("acp-share-note"), { target: { value: "  给小明  " } });
    fireEvent.click(screen.getByTestId("acp-share-create"));
    await waitFor(() => {
      expect((screen.getByTestId("acp-share-link") as HTMLInputElement).value).toBe(LINK);
    });
    const calls = callsOf(fetchMock);
    expect(calls[0][0]).toBe("http://127.0.0.1:9910/shares");
    expect(calls[0][1]).toEqual({ scope: "sandbox", ttl_secs: 3_600, max_activations: 1, note: "给小明" });

    await pickOption("acp-share-scope", "工作区目录");
    await pickOption("acp-share-ttl", "24 小时");
    fireEvent.click(screen.getByTestId("acp-share-create"));
    await waitFor(() => {
      expect(callsOf(fetchMock)).toHaveLength(2);
    });
    expect(callsOf(fetchMock)[1][1]).toEqual({
      scope: "workspace",
      ttl_secs: 86_400,
      max_activations: 1,
      note: "给小明",
    });
  });

  it("生成后可复制可发送：onSendLink 收到链接原文并关闭弹层", async () => {
    stubCreatefetch(LINK);
    const onSendLink = vi.fn(async () => undefined);
    const onOpenChange = vi.fn();
    render(<ShareCreateDialog open onOpenChange={onOpenChange} onSendLink={onSendLink} />);
    fireEvent.click(screen.getByTestId("acp-share-create"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-share-link")).toBeTruthy();
    });
    expect(screen.getByTestId("acp-share-copy")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-share-send"));
    await waitFor(() => {
      expect(onSendLink).toHaveBeenCalledWith(LINK);
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });

  it("admin 端点未登记且本机自描述缺失：显式引导且生成入口停用", async () => {
    useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p" } });
    render(<ShareCreateDialog open onOpenChange={vi.fn()} />);
    expect(await screen.findByTestId("acp-share-admin-missing")).toBeTruthy();
    expect((screen.getByTestId("acp-share-create") as HTMLButtonElement).disabled).toBe(true);
  });

  it("本机自描述兜底：未登记时自动发现 admin 端点并可创建", async () => {
    const fetchMock = stubCreatefetch(LINK);
    const spy = vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue({
      adminUrl: "http://127.0.0.1:8123",
      token: "auto-tok",
      peer: "peerLOCAL",
      agentName: "home-agent",
      writtenAtUnix: 1_725_700_000,
    });
    try {
      useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p" } });
      render(<ShareCreateDialog open onOpenChange={vi.fn()} />);
      expect(await screen.findByTestId("acp-share-local-auto")).toBeTruthy();
      expect(screen.queryByTestId("acp-share-admin-missing")).toBeNull();
      expect((screen.getByTestId("acp-share-create") as HTMLButtonElement).disabled).toBe(false);
      fireEvent.click(screen.getByTestId("acp-share-create"));
      await waitFor(() => {
        expect((screen.getByTestId("acp-share-link") as HTMLInputElement).value).toBe(LINK);
      });
      expect(callsOf(fetchMock)[0][0]).toBe("http://127.0.0.1:8123/shares");
    } finally {
      spy.mockRestore();
    }
  });
});

describe("ShareCreateDialog 多工作区定向（2026-09-07 加法）", () => {
  it("scope=workspace 且 agent 有工作区清单：POST 体携带定向 workspace id", async () => {
    const fetchMock = vi.fn(async (url: string, _init?: RequestInit) => {
      if (String(url).endsWith("/workspaces")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            workspaces: [
              { id: "ws1", name: "p2p", dir: "/home/me/p2p" },
              { id: "ws2", name: "blog", dir: "/home/me/blog" },
            ],
          }),
        };
      }
      return {
        ok: true,
        status: 200,
        json: async () => ({ share_id: "sid-9", token: TOKEN32, link: LINK, expires_at_unix: 3_000 }),
      };
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<ShareCreateDialog open onOpenChange={vi.fn()} />);
    await pickOption("acp-share-scope", "工作区目录");
    const picker = await screen.findByTestId("acp-share-workspace");
    expect(picker).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-share-create"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-share-link")).toBeTruthy();
    });
    const posts = callsOf(fetchMock);
    expect(posts[0][1]).toMatchObject({ scope: "workspace", workspace: "ws1" });
  });
});
