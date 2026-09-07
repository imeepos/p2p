// 验收 4（§3.2 endpoint 流 + §3.4）：endpoint 添加 → 测试连接 → 保存 →
// 发起会话断言；失败路径显式呈现断言（测试未通过可保存但显警告徽标）。
// 验收 5（§3.3）：详情抽屉五块渲染断言；权限档变更即时落存断言。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole, createMockWsFactory } = await import("@/acp/mock-acp-ws");
const { useAcpStore } = await import("@/acp/acp-store");
const { useEndpointMetaStore } = await import("@/acp/endpoint-meta");
const { setWsFactory } = await import("@/acp/ws-factory");
const { resetToastDedupForTest } = await import("@/components/feedback/toast");
const { toast } = await import("sonner");

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { AppToaster } from "@/components/ui/sonner";
import { ThemeProvider } from "@/theme/theme-provider";
import type { WsLike } from "@/acp/ws-factory";
import { AgentSection } from "./agent-section";

// mock 白名单对齐真实契约：peer 用合法 base58（对话框同口径校验）
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function renderSection(entry = "/contacts") {
  return render(
    <MemoryRouter initialEntries={[entry]}>
      <ConfirmProvider>
        <ThemeProvider>
          <Routes>
            <Route path="/contacts" element={<AgentSection />} />
            <Route path="/chat" element={<div data-testid="chat-probe" />} />
          </Routes>
          {/* 全局 toast 面随真实应用挂载：失败 toast 断言依赖它在场 */}
          <AppToaster position="top-right" />
        </ThemeProvider>
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
  resetToastDedupForTest();
  radixStubs();
});

afterEach(() => {
  act(() => {
    toast.dismiss();
  });
  setWsFactory(null);
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
    // 失败轻提示：toast 上墙且带关闭详情（abnormal + code），可复制排查
    await waitFor(() => {
      const text = document.querySelector("[data-sonner-toast]")?.textContent ?? "";
      expect(text).toContain("连接失败");
      expect(text).toContain("abnormal (code 1006)");
    });
    // 点击 toast 本体可关闭（AppToaster 事件委托）
    fireEvent.click(document.querySelector("[data-sonner-toast] [data-title]")!);
    await waitFor(() =>
      expect(document.querySelector('[data-sonner-toast]:not([data-removed="true"])')).toBeNull(),
    );
    // 未通过测试允许保存，行内显警告徽标（§3.2）
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() => expect(screen.getByTestId("contact-agent-" + savedId())).toBeTruthy());
    await waitFor(() =>
      expect(screen.getByTestId("contact-agent-warn-" + savedId())).toBeTruthy(),
    );
  });

  it("token 分径：保存可空（先存后连）；连接必填且错误与字段同屏（自动展开高级区）", async () => {
    setWsFactory(createMockWsFactory());
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-agent-add"));
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-endpoint-wsurl"), {
      target: { value: "ws://127.0.0.1:8787" },
    });
    fireEvent.change(screen.getByTestId("contacts-endpoint-peer"), {
      target: { value: PEER },
    });
    // 显式清空 token：同文件前序用例的草稿会跨用例残留（resetConsoleState 不清 draft）
    fireEvent.change(screen.getByTestId("contacts-endpoint-token"), {
      target: { value: "" },
    });
    // 连接路径：缺 token 显式报错，高级区自动展开（错误与字段同屏，可观测）
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    expect(screen.getByTestId("contacts-endpoint-error-tokenRequired")).toBeTruthy();
    expect(screen.getByTestId("contacts-endpoint-advanced").hidden).toBe(false);
    // 保存路径：token 可空，先存后连（§3.2 原始口径，2026-09-07 裁决回归）
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() =>
      expect(screen.queryByTestId("contacts-endpoint-dialog")).toBeNull(),
    );
    await waitFor(() => expect(screen.getByTestId("contact-agent-" + savedId())).toBeTruthy());
  });

  it("console ready 时弹窗顶部显本机自动接入提示（引导远端场景，无需手填本机）", async () => {
    setWsFactory(createMockWsFactory());
    useAcpStore.setState({
      console: {
        phase: "ready",
        wsUrl: "ws://127.0.0.1:9987",
        token: "local-token",
        statusUrl: "http://127.0.0.1:9987",
        restarts: 0,
      },
    });
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-agent-add"));
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    expect(screen.getByTestId("contacts-endpoint-local-hint")).toBeTruthy();
  });

  /** 打开弹窗并按测试所需填主字段（peer 必填口径） */
  async function openAndFill(wsFactory: Parameters<typeof setWsFactory>[0]) {
    setWsFactory(wsFactory);
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
  }

  it("测试首拨进重连即判失败：立即断开止损并 toast，不空转重试", async () => {
    // 非终态关断码（3000 abnormal）且未 open：客户端视角进 reconnecting
    await openAndFill(() => {
      const sock: WsLike = {
        send: () => {},
        close: () => {},
        onopen: null,
        onclose: null,
        onerror: null,
        onmessage: null,
      };
      window.setTimeout(() => sock.onclose?.({ code: 3000, reason: "abnormal-drop" }), 0);
      return sock;
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    await waitFor(() => expect(useAcpStore.getState().phase).toBe("idle"));
    expect(screen.getByTestId("contacts-endpoint-test-failed")).toBeTruthy();
    await waitFor(() =>
      expect(
        document.querySelector('[data-sonner-toast]:not([data-removed="true"])')?.textContent ?? "",
      ).toContain("连接失败"),
    );
  });

  it("拨号悬挂超时兜底：转失败断开解除 loading，不无限「连接中…」", async () => {
    await openAndFill(() => ({
      send: () => {},
      close: () => {},
      onopen: null,
      onclose: null,
      onerror: null,
      onmessage: null,
    }));
    vi.useFakeTimers();
    try {
      fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
      expect(useAcpStore.getState().phase).toBe("connecting");
      await act(async () => {
        await vi.advanceTimersByTimeAsync(12_500);
      });
    } finally {
      vi.useRealTimers();
    }
    expect(useAcpStore.getState().phase).toBe("idle");
    expect(screen.getByTestId("contacts-endpoint-test-failed")).toBeTruthy();
    expect(
      document.querySelector('[data-sonner-toast]:not([data-removed="true"])')?.textContent ?? "",
    ).toContain("连接失败");
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

  it("R2-23/R2-24 深链 /contacts?agentDetail=<id> 直开详情抽屉", async () => {
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
    renderSection("/contacts?agentDetail=ep-1");
    await waitFor(() => expect(screen.getByTestId("contacts-agent-drawer")).toBeTruthy());
    expect(screen.getByTestId("contacts-agent-drawer-title").textContent).toContain("编码助手");
    // 权限面板所在的策略块可达（R2-24 深链落点）
    expect(screen.getByTestId("contacts-drawer-block-policy")).toBeTruthy();
  });

  it("深链未命中已登记端点：不开抽屉（只读参数不改现有行为）", async () => {
    useAcpStore.setState({ saved: [] });
    renderSection("/contacts?agentDetail=ghost");
    await act(async () => {});
    expect(screen.queryByTestId("contacts-agent-drawer")).toBeNull();
  });
});
