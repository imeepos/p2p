// ACP 控制台组件测试：整链走 mock WS（VITE_MOCK_IPC=1 与 dev 同实现），
// 覆盖连接握手能力位、流式气泡、思考折叠、会话侧栏生命周期与断线重连提示。
// 交互面（工具/权限/配置/用量/续连/目录）在 acp-view-interactions.test.tsx。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { resetFixtures, sendPrompt } = await import("./acp-view-test-utils");
await import("@/i18n");

const DRAFT = { wsUrl: "ws://127.0.0.1:8787", token: "mock-token", peer: "mock-peer" };

function draftEndpoint(peer = DRAFT.peer) {
  return { ...DRAFT, peer };
}

beforeEach(() => {
  resetFixtures();
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

  it("token 错误：1006 空 reason 首轮即停转离线并提示检查 token（真机 R3a）", async () => {
    useAcpStore.setState({ draft: draftEndpoint() });
    useAcpStore.getState().setDraft({ token: "wrong-token" });
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-close-info").textContent).toContain("检查 token");
    });
    await waitFor(() => {
      expect(screen.getByTestId("acp-phase-badge").textContent).toContain("离线");
    });
    expect(screen.getByTestId("acp-offline-reason").textContent).toContain("检查 token");
    expect(screen.queryByTestId("acp-reconnect-notice")).toBeNull();
  });

  it("token 输入框密码型回显", () => {
    render(<AcpView />);
    expect((screen.getByTestId("acp-input-token") as HTMLInputElement).type).toBe("password");
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
      expect(screen.getByTestId("acp-transcript").textContent).toContain("正常结束");
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
    await sendPrompt();
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
      // 有 reason 的异常断流走自动重连（1006 空 reason 已按鉴权速断）
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