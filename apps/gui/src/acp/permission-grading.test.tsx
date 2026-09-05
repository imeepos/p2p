// F4 行为测试（P1）：权限应答分级。allow_once 单击即应答但样式降为次级；
// reject 档 destructive 强调与 allow 可区分；allow_always 必须经一次显式
// 确认弹框（取消不生效）才回 selected outcome。
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole } = await import("./mock-acp-ws");
const { renderConnected, permissionId, sendPrompt, resetFixtures } = await import(
  "./acp-view-test-utils"
);
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

// as const 收窄 kind 字面量，展开回可变数组喂 configure（与既有用例同款）
const SCRIPT = [
  { kind: "permission", toolKind: "execute", title: "Run tests" },
  { kind: "stop", reason: "end_turn" },
] as const;

describe("AcpView permission grading", () => {
  it("allow_once 单击即应答，样式为次级（secondary）而非主色", async () => {
    mockAcpConsole.configure({ promptScript: [...SCRIPT] });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    const allow = await screen.findByTestId("acp-permission-option-" + id + "-allow-once");
    expect(allow.className).toContain("bg-secondary");
    expect(allow.getAttribute("data-perm-action")).toBe("allow-once");
    fireEvent.click(allow);
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已批准");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "selected", optionId: "allow-once" },
    });
  });

  it("reject 选项 destructive 强调，data-perm-action 与 allow 档可区分", async () => {
    mockAcpConsole.configure({ promptScript: [...SCRIPT] });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    const reject = await screen.findByTestId("acp-permission-option-" + id + "-reject-once");
    expect(reject.className).toContain("text-destructive");
    expect(reject.getAttribute("data-perm-action")).toBe("reject");
    expect(reject.className).not.toContain("bg-secondary");
  });

  it("allow_always 单击只弹确认框，取消不生效，确认后才回 selected outcome", async () => {
    mockAcpConsole.configure({
      promptScript: [
        {
          kind: "permission",
          toolKind: "execute",
          title: "Run tests",
          options: [
            { optionId: "allow-once", name: "Allow once", kind: "allow_once" },
            { optionId: "allow-always", name: "Allow always", kind: "allow_always" },
            { optionId: "reject-once", name: "Deny once", kind: "reject_once" },
          ],
        },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    const always = await screen.findByTestId("acp-permission-option-" + id + "-allow-always");
    expect(always.getAttribute("data-perm-action")).toBe("allow-always");
    // 单击：只弹确认框，不产生应答
    fireEvent.click(always);
    expect(await screen.findByText("确认始终允许？")).toBeTruthy();
    expect(mockAcpConsole.responses.find((r) => r.id === id)).toBeUndefined();
    expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("等待处理");
    // 取消：不生效，仍 pending
    fireEvent.click(screen.getByText("取消"));
    await waitFor(() => {
      expect(screen.queryByText("确认始终允许？")).toBeNull();
    });
    expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("等待处理");
    expect(mockAcpConsole.responses.find((r) => r.id === id)).toBeUndefined();
    // 再次单击并确认：两步后才应答
    fireEvent.click(screen.getByTestId("acp-permission-option-" + id + "-allow-always"));
    await screen.findByText("确认始终允许？");
    fireEvent.click(screen.getByText("始终允许"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已批准");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "selected", optionId: "allow-always" },
    });
  });
});
