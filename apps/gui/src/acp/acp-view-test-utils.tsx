// AcpView 组件测试共用夹具：mock 连接、会话建立、prompt 发送与 Radix 下拉点选。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, vi } from "vitest";

// mockAcpConsole/useAcpStore 仅供本文件内部使用；测试文件各自直接动态导入，
// 避免把匿名类类型导出到 d.ts（TS4094）。
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");

export const DRAFT = { wsUrl: "ws://127.0.0.1:8787", token: "mock-token", peer: "mock-peer" };

export function resetFixtures() {
  localStorage.clear();
  mockAcpConsole.reset();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ draft: { ...DRAFT } });
  // Radix Select 在 jsdom 下需要指针捕获/滚动桩（官方已知测试前提）
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

export async function renderConnected() {
  const { AcpView } = await import("./acp-view");
  render(<AcpView />);
  fireEvent.click(screen.getByTestId("acp-connect"));
  await waitFor(() => {
    expect(screen.getByTestId("acp-capabilities-card")).toBeTruthy();
  });
}

export async function newSession() {
  fireEvent.click(screen.getByTestId("acp-session-new"));
  await waitFor(() => {
    expect(screen.getByTestId("acp-session-row-s-001")).toBeTruthy();
  });
}

export async function sendPrompt(text = "hi") {
  await newSession();
  fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: text } });
  fireEvent.click(screen.getByTestId("acp-composer-send"));
}

// Radix 只认 pointerType=mouse 的 pointerdown（与 chat 同款辅助）
export async function pickOption(triggerTestId: string, name: string) {
  fireEvent.pointerDown(screen.getByTestId(triggerTestId), {
    button: 0,
    ctrlKey: false,
    pointerType: "mouse",
  });
  const option = await screen.findByRole("option", { name });
  fireEvent.pointerUp(option, { button: 0, pointerType: "mouse" });
  fireEvent.click(option, { button: 0, pointerType: "mouse" });
}

export async function permissionId(): Promise<number> {
  await vi.waitFor(() => {
    expect(mockAcpConsole.permissionRequests.length).toBeGreaterThan(0);
  });
  return mockAcpConsole.permissionRequests[0].id;
}