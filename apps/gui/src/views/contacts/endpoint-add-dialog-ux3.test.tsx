import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// UX3 表单收敛验收：主字段 = 发现清单下拉（禁自由文本）；高级折叠默认收起且以
// 本机 console 值预填；「添加并开始对话」= 保存+测试连接+连接+跳转会话一条龙。
// 既有收藏/历史值/保存语义不退化由 contacts-forms.test.tsx 继续覆盖。
vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole: mockAcpWs, createMockWsFactory } = await import("@/acp/mock-acp-ws");
const { setWsFactory } = await import("@/acp/ws-factory");
const { useAcpStore } = await import("@/acp/acp-store");
const { useEndpointMetaStore } = await import("@/acp/endpoint-meta");

import "@/i18n";
import { EndpointAddDialog } from "./endpoint-add-dialog";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const CONSOLE_STATUS = {
  phase: "connected" as const,
  wsUrl: "ws://127.0.0.1:8787",
  token: "mock-console-token",
  statusUrl: "http://127.0.0.1:8788",
};

function radixStubs(): void {
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

function renderDialog() {
  return render(
    <MemoryRouter initialEntries={["/contacts"]}>
      <Routes>
        <Route
          path="/contacts"
          element={<EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />}
        />
        <Route path="/chat" element={<div data-testid="chat-probe" />} />
      </Routes>
    </MemoryRouter>,
  );
}

async function pickTarget(name: string): Promise<void> {
  // F25：目标选择器为统一关联选择器（点击展开），选择即回填 peer
  fireEvent.click(screen.getByTestId("contacts-endpoint-target"));
  const option = await screen.findByRole("option", { name: new RegExp(name) });
  fireEvent.click(option, { button: 0, pointerType: "mouse" });
}

beforeEach(() => {
  localStorage.clear();
  mockAcpWs.reset();
  mockAcpWs.configure({ token: "mock-console-token", peers: [PEER] });
  useEndpointMetaStore.getState().resetForTest();
  useAcpStore.getState().resetConsoleState();
  // saved/draft 为持久化档（resetConsoleState 不清）：显式复位保用例隔离
  useAcpStore.setState({
    saved: [],
    draft: { wsUrl: "ws://127.0.0.1:8787", token: "", peer: "" },
    console: CONSOLE_STATUS,
    directory: [
      { peer: PEER, name: "本机助手", scope: "sandbox", source: "discovered", addrs: [], via: "mdns" },
    ],
  });
  radixStubs();
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ ok: true, json: async () => ({ peers: [] }) })),
  );
});

describe("endpoint 添加表单收敛（UX3）", () => {
  it("WS 地址主字段默认展开；目标选择器辅助填充：候选来自发现面，选择即回填", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    // F25 主字段：wsUrl 展开可见（无需展开高级区）
    expect((screen.getByTestId("contacts-endpoint-wsurl") as HTMLInputElement).value).toBe(
      "ws://127.0.0.1:8787",
    );
    await pickTarget("本机助手");
    // 选中即回填（触发器呈现候选名）；选择器是触发器（非可自由输入的文本框）
    expect(screen.getByTestId("contacts-endpoint-target").textContent).toContain("本机助手");
    expect(screen.getByTestId("contacts-endpoint-target").tagName).not.toBe("INPUT");
  });

  it("高级折叠默认收起且以本机 console 值预填；展开后字段可见可改", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    const advanced = screen.getByTestId("contacts-endpoint-advanced");
    expect(advanced.getAttribute("hidden")).not.toBeNull();
    // 预填：console 连接面（保存草稿未动过的字段被覆盖）
    fireEvent.click(screen.getByTestId("contacts-endpoint-advanced-toggle"));
    expect(advanced.getAttribute("hidden")).toBeNull();
    expect((screen.getByTestId("contacts-endpoint-wsurl") as HTMLInputElement).value).toBe(
      "ws://127.0.0.1:8787",
    );
    expect((screen.getByTestId("contacts-endpoint-token") as HTMLInputElement).value).toBe(
      "mock-console-token",
    );
  });

  it("「添加并开始对话」一条龙：保存 + 连接 + 跳转 /chat?agent=<id>", async () => {
    setWsFactory(createMockWsFactory());
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    await pickTarget("本机助手");
    fireEvent.click(screen.getByTestId("contacts-endpoint-add-open"));
    // 跳转会话
    await waitFor(() => expect(screen.getByTestId("chat-probe")).toBeTruthy());
    // 保存 + 连接语义：收藏在册、phase 进入连接期/在线
    const saved = useAcpStore.getState().saved;
    expect(saved).toHaveLength(1);
    expect(saved[0]!.peer).toBe(PEER);
    await waitFor(() => {
      const phase = useAcpStore.getState().phase;
      expect(phase === "connecting" || phase === "online").toBe(true);
    });
    await waitFor(() => expect(useAcpStore.getState().phase).toBe("online"));
  });

  it("目标未选点一条龙：targetRequired 显式拦截，不保存不跳转", async () => {
    useAcpStore.setState({ directory: [] });
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contacts-endpoint-add-open"));
    await waitFor(() =>
      expect(screen.getByTestId("contacts-endpoint-target-error-targetRequired")).toBeTruthy(),
    );
    expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy();
    expect(useAcpStore.getState().saved).toHaveLength(0);
  });
});
