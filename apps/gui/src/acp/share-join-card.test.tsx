// guest「用链接加入」测试：POST /connect-share 调用契约（mock console）、
// 成功进连接目录（scope 徽章=分享 scope）、denied 原因显式上浮、非法链接不发请求。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareJoinCard } = await import("./components/share-join-card");
const { useAcpStore } = await import("./acp-store");
const { resetFixtures } = await import("./acp-view-test-utils");
await import("@/i18n");

const PEER = "peerX";
const LINK = "dsh-acp-share://v1?peer=" + PEER + "&token=" + "a".repeat(32);

function stubConsole(body: unknown, ok = true) {
  const fetchMock = vi.fn(async () => ({ ok, status: ok ? 200 : 409, json: async () => body }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

beforeEach(() => {
  resetFixtures();
  // resetConsoleState 不清连接目录（跨重连存活语义），用例隔离需显式清
  useAcpStore.setState({ directory: [] });
  useAcpStore.getState().setDraft({
    statusUrl: "http://127.0.0.1:9900",
    token: "console-tok",
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function pasteAndJoin(link: string) {
  fireEvent.change(screen.getByTestId("acp-share-join-input"), { target: { value: link } });
  fireEvent.click(screen.getByTestId("acp-share-join-action"));
}

describe("ShareJoinCard guest 导入契约", () => {
  it("成功：POST {link} 到 /connect-share，目录出现该 peer 且 scope=分享 scope", async () => {
    const fetchMock = stubConsole({ ok: true, peer: PEER, scope: "workspace" });
    render(<ShareJoinCard />);
    pasteAndJoin(LINK);
    await waitFor(() => {
      expect(screen.getByTestId("acp-share-join-ok")).toBeTruthy();
    });
    const [url, init] = fetchMock.mock.calls[0] as unknown as [
      string,
      { method: string; body: string; headers: Record<string, string> },
    ];
    expect(url).toBe("http://127.0.0.1:9900/connect-share");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ link: LINK });
    expect(init.headers.Authorization).toBe("Bearer console-tok");
    const entry = useAcpStore.getState().directory.find((e) => e.peer === PEER);
    expect(entry?.scope).toBe("workspace");
    expect(entry?.source).toBe("manual");
  });

  it("denied：ok=false + code/reason 显式展示，不入目录", async () => {
    stubConsole({ code: "share-expired", reason: "expired at 123" }, false);
    render(<ShareJoinCard />);
    pasteAndJoin(LINK);
    const denied = await screen.findByTestId("acp-share-join-denied");
    expect(denied.textContent).toContain("share-expired");
    expect(denied.textContent).toContain("expired at 123");
    expect(useAcpStore.getState().directory.find((e) => e.peer === PEER)).toBeUndefined();
  });

  it("非法链接：行内报错且不发请求；console 未配置显式引导", async () => {
    const fetchMock = stubConsole({ ok: true, peer: PEER });
    render(<ShareJoinCard />);
    pasteAndJoin("https://example.com/share");
    expect(await screen.findByTestId("acp-share-join-invalid")).toBeTruthy();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(useAcpStore.getState().directory).toHaveLength(0);

    useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p" } });
    pasteAndJoin(LINK);
    expect(await screen.findByTestId("acp-share-join-need-console")).toBeTruthy();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
