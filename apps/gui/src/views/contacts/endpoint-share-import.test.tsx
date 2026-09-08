// 弹窗分享链接导入流（§8 guest 导入的弹窗形态）：粘贴 dsh-acp-share:// 链接 →
// 导入成功落 saved endpoint（稳定 id acp-share-<peer>）→ 行内出现该 agent；
// needConsole 显式提示（本机 agent 未就绪不静默失败）。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { useAcpStore } = await import("@/acp/acp-store");
const { useEndpointMetaStore } = await import("@/acp/endpoint-meta");

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { ThemeProvider } from "@/theme/theme-provider";
import { AgentSection } from "./agent-section";

// mock 白名单对齐真实契约：peer 用合法 base58（对话框同口径校验）
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const LINK = "dsh-acp-share://v1?peer=" + PEER + "&token=" + "a".repeat(32);

function renderSection() {
  return render(
    <MemoryRouter initialEntries={["/contacts"]}>
      <ConfirmProvider>
        <ThemeProvider>
          <Routes>
            <Route path="/contacts" element={<AgentSection />} />
            <Route path="/chat" element={<div data-testid="chat-probe" />} />
          </Routes>
        </ThemeProvider>
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

async function openDialog() {
  fireEvent.click(screen.getByTestId("contacts-agent-add"));
  await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
}

beforeEach(() => {
  localStorage.clear();
  useEndpointMetaStore.getState().resetForTest();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ saved: [] });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("弹窗分享链接导入（§8 guest）", () => {
  it("粘贴链接导入成功：落 acp-share-<peer> saved endpoint，行内出现该 agent", async () => {
    useAcpStore.setState({
      console: {
        phase: "connected",
        wsUrl: "ws://127.0.0.1:9987",
        token: "local-token",
        statusUrl: "http://127.0.0.1:9987",
      },
    });
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: true, json: async () => ({ ok: true, peer: PEER, scope: "sandbox" }) })),
    );
    renderSection();
    await openDialog();
    fireEvent.change(screen.getByTestId("contacts-endpoint-wsurl"), {
      target: { value: "看看这个 " + LINK },
    });
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-share-import")).toBeTruthy(),
    );
    fireEvent.click(screen.getByTestId("contacts-endpoint-share-import-action"));
    await waitFor(() =>
      expect(screen.queryByTestId("contacts-endpoint-dialog")).toBeNull(),
    );
    const saved = useAcpStore.getState().saved;
    const shared = saved.find((e) => e.endpointId === "acp-share-" + PEER);
    expect(shared?.peer).toBe(PEER);
    // 连接面 = 本机 console（经本机 agent 桥接拨号）
    expect(shared?.wsUrl).toBe("ws://127.0.0.1:9987");
    expect(shared?.token).toBe("local-token");
    await waitFor(() =>
      expect(screen.getByTestId("contact-agent-acp-share-" + PEER)).toBeTruthy(),
    );
  });

  it("本机 agent 未就绪：needConsole 显式提示，弹窗保持打开不静默", async () => {
    // 显式清空草稿连接面：upsertSaved 会写 draft（跨用例残留），不清则 join 走通
    useAcpStore.setState({ draft: { wsUrl: "", token: "", statusUrl: "", peer: "", alias: "" } });
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: true, json: async () => ({ ok: true }) })),
    );
    renderSection();
    await openDialog();
    fireEvent.change(screen.getByTestId("contacts-endpoint-wsurl"), {
      target: { value: LINK },
    });
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-share-import")).toBeTruthy(),
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId("contacts-endpoint-share-import-action"));
    });
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-share-import-need-console")).toBeTruthy(),
    );
    expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy();
  });
});