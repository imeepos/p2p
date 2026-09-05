// 验收 4（§3.2 endpoint 流 + §3.4）：endpoint 添加 → 测试连接 → 保存 →
// 发起会话断言；失败路径显式呈现断言（测试未通过可保存但显警告徽标）。
// 验收 5（§3.3）：详情抽屉五块渲染断言；权限档变更即时落存断言。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole, createMockWsFactory } = await import("@/acp/mock-acp-ws");
const { useAcpStore } = await import("@/acp/acp-store");
const { useEndpointMetaStore } = await import("@/acp/endpoint-meta");
const { setWsFactory } = await import("@/acp/ws-factory");

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { AgentSection } from "./agent-section";

// mock 白名单对齐真实契约：peer 用合法 base58（对话框同口径校验）
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function renderSection() {
  return render(
    <MemoryRouter initialEntries={["/contacts"]}>
      <ConfirmProvider>
        <Routes>
          <Route path="/contacts" element={<AgentSection />} />
          <Route path="/chat" element={<div data-testid="chat-probe" />} />
        </Routes>
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

function radixStubs() {
  Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "hasPointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "releasePointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  mockAcpConsole.configure({ peers: [PEER] });
  useEndpointMetaStore.getState().resetForTest();
  useAcpStore.getState().resetConsoleState();
  radixStubs();
});

describe("endpoint 添加 → 测试连接 → 保存 → 发起会话（P2 验收 4）", () => {
function savedId(): string {
  const saved = useAcpStore.getState().saved;
  expect(saved.length).toBeGreaterThan(0);
  return saved[0]!.endpointId!;
}

  it("全流程：测试连接通过后保存，行内出现该 endpoint 且可发消息", async () => {
    setWsFactory(createMockWsFactory());
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-agent-add"));
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-endpoint-wsurl"), {
      target: { value: "ws://127.0.0.1:8787" },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-token"), {
      target: { value: "mock-token" },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-peer"), {
      target: { value: PEER },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-alias"), {
      target: { value: "编码助手" },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-test-ok")).toBeTruthy(),
    );
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() =>
      expect(screen.queryByTestId("contacts-endpoint-dialog")).toBeNull(),
    );
    await waitFor(() => expect(screen.getByTestId("contact-agent-" + savedId())).toBeTruthy());
    expect(
      screen.getByTestId("contact-agent-message-" + savedId()).getAttribute("href"),
    ).toBe("/chat?agent=" + savedId());
    // 存档语义：localStorage 收藏（endpoint-storage 键）含该端点
    const stored = JSON.parse(localStorage.getItem("p2p-gui-acp-endpoints") ?? "{}");
    expect((stored.saved ?? []).some((e: { endpointId?: string }) => e.endpointId === savedId())).toBe(true);
  });

  it("失败路径：错误 token 测试失败显式呈现；允许保存但行内显警告徽标", async () => {
    setWsFactory(createMockWsFactory());
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-agent-add"));
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-endpoint-wsurl"), {
      target: { value: "ws://127.0.0.1:8787" },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-token"), {
      target: { value: "wrong-token" },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-peer"), {
      target: { value: PEER },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-test-failed")).toBeTruthy(),
    );
    // 未通过测试允许保存，行内显警告徽标（§3.2）
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() => expect(screen.getByTestId("contact-agent-" + savedId())).toBeTruthy());
    await waitFor(() =>
      expect(screen.getByTestId("contact-agent-warn-" + savedId())).toBeTruthy(),
    );
  });
});

describe("详情抽屉五块与权限档变更（P2 验收 5）", () => {
  async function openDrawer(online = false) {
    if (online) setWsFactory(createMockWsFactory());
    useAcpStore.setState({
      saved: [
        {
          wsUrl: "ws://127.0.0.1:8787",
          token: "mock-token",
          peer: PEER,
          alias: "编码助手",
          endpointId: "ep-1",
        },
      ],
    });
    if (online) {
      useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:8787", token: "mock-token", peer: PEER, endpointId: "ep-1" } });
      useAcpStore.getState().connect();
      await waitFor(() => expect(useAcpStore.getState().phase).toBe("online"));
    }
    renderSection();
    fireEvent.click(screen.getByTestId("contact-agent-detail-ep-1"));
    await waitFor(() => expect(screen.getByTestId("contacts-agent-drawer")).toBeTruthy());
  }

  it("五块渲染：连接/能力/权限策略/会话/危险区齐备，危险区含停用与删除", async () => {
    await openDrawer();
    for (const id of ["connection", "capabilities", "policy", "sessions", "dangerZone"]) {
      expect(screen.getByTestId("contacts-drawer-block-" + id)).toBeTruthy();
    }
    expect(screen.getByTestId("contacts-drawer-test")).toBeTruthy();
    expect(screen.getByTestId("contacts-drawer-edit")).toBeTruthy();
    expect(screen.getByTestId("contacts-agent-drawer-disable")).toBeTruthy();
    expect(screen.getByTestId("contacts-agent-drawer-remove")).toBeTruthy();
    // 会话块：未连接/无会话显空态提示，不静默空白
    expect(screen.getByTestId("contacts-agent-sessions-empty")).toBeTruthy();
  });

  it("在线时会话块列出历史会话，点击跳 /chat?agent= 并载入 transcript（resume）", async () => {
    await openDrawer(true);
    await act(async () => {
      await useAcpStore.getState().newSession();
    });
    await waitFor(() => expect(screen.getByTestId("contacts-agent-session-s-001")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contacts-agent-session-s-001"));
    await waitFor(() => expect(screen.getByTestId("chat-probe")).toBeTruthy());
  });

  it("权限档变更：编辑器改动即时写入 endpoint 元数据（对后续请求生效）", async () => {
    await openDrawer();
    const trigger = screen.getByTestId("contacts-policy-default-execute");
    fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: "mouse" });
    const option = await screen.findByRole("option", { name: "拒绝" });
    fireEvent.pointerUp(option, { button: 0, pointerType: "mouse" });
    fireEvent.click(option, { button: 0, pointerType: "mouse" });
    await waitFor(() =>
      expect(useEndpointMetaStore.getState().policies["ep-1"]?.defaults.execute).toBe("deny"),
    );
    const raw = JSON.parse(localStorage.getItem("p2p-gui-acp-endpoint-meta") ?? "{}");
    expect(raw.policies?.["ep-1"]?.defaults?.execute).toBe("deny");
  });
});
