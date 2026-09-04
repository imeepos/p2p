// ACP 控制台组件测试：整链走 mock WS（VITE_MOCK_IPC=1 与 dev 同实现），
// 覆盖连接握手能力位、流式气泡、思考折叠、会话侧栏生命周期与断线重连提示。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { AcpView } = await import("./acp-view");
await import("@/i18n");

const DRAFT = { wsUrl: "ws://127.0.0.1:8787", token: "mock-token", peer: "mock-peer" };

function draftEndpoint(peer = DRAFT.peer) {
  return { ...DRAFT, peer };
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ draft: { ...DRAFT } });
});

async function renderConnected() {
  render(<AcpView />);
  fireEvent.click(screen.getByTestId("acp-connect"));
  await waitFor(() => {
    expect(screen.getByTestId("acp-capabilities-card")).toBeTruthy();
  });
}

async function newSession() {
  fireEvent.click(screen.getByTestId("acp-session-new"));
  await waitFor(() => {
    expect(screen.getByTestId("acp-session-row-s-001")).toBeTruthy();
  });
}

describe("AcpView connection", () => {
  it("连接成功：phase 在线 + initialize 能力位如实展示", async () => {
    await renderConnected();
    const card = screen.getByTestId("acp-capabilities-card");
    expect(card.textContent).toContain("mock-agent");
    expect(card.textContent).toContain("支持");
    expect(card.textContent).toContain("不支持");
    expect(screen.getByTestId("acp-phase-badge").textContent).toContain("在线");
  });

  it("token 错误：4403 denied 可观察，phase 转离线", async () => {
    useAcpStore.setState({ draft: draftEndpoint() });
    useAcpStore.getState().setDraft({ token: "wrong-token" });
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-close-info")).toBeTruthy();
    });
    expect(screen.getByTestId("acp-close-info").textContent).toContain("4403");
    expect(screen.getByTestId("acp-phase-badge").textContent).toContain("离线");
  });

  it("端点可保存并回填（手动添加）", async () => {
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-save-endpoint"));
    expect(screen.getByTestId("acp-endpoint-fill-mock-peer")).toBeTruthy();
  });
});

describe("AcpView transcript", () => {
  it("prompt 回放：思考折叠面板默认收起，展开可见内容；气泡含结束原因", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: "hi" } });
    fireEvent.click(screen.getByTestId("acp-composer-send"));
    const bubble = await screen.findByText("Hello from the mock agent.");
    expect(bubble).toBeTruthy();
    const toggle = screen.getByTestId("acp-thought-toggle-2");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("thinking through the request")).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByTestId("acp-transcript").textContent).toContain("end_turn");
    });
  });

  it("流式分块聚合到同一气泡", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "message", text: "部分A" },
        { kind: "message", text: "部分B" },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await newSession();
    fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: "hi" } });
    fireEvent.click(screen.getByTestId("acp-composer-send"));
    expect(await screen.findByText("部分A部分B")).toBeTruthy();
  });
});

describe("AcpView sessions", () => {
  it("会话侧栏生命周期：新建出现、关闭移除", async () => {
    await renderConnected();
    await newSession();
    fireEvent.click(screen.getByTestId("acp-session-close-s-001"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-session-row-s-001")).toBeNull();
    });
  });

  it("断线自动重连提示，会话清单跨重连存活", async () => {
    await renderConnected();
    await newSession();
    act(() => {
      mockAcpConsole.dropAll(1000, "agent-stream-dropped");
    });
    await waitFor(
      () => {
        expect(screen.getByTestId("acp-reconnect-notice")).toBeTruthy();
      },
      { timeout: 5_000 },
    );
    // mock 重连（base 1s 退避）成功后能力位恢复，会话仍在侧栏
    await waitFor(
      () => {
        expect(screen.getByTestId("acp-capabilities-card")).toBeTruthy();
      },
      { timeout: 10_000 },
    );
    expect(screen.getByTestId("acp-session-row-s-001")).toBeTruthy();
    expect(screen.getByTestId("acp-phase-badge").textContent).toContain("在线");
  });
});
