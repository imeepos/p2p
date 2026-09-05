// §3.3 权限策略自动应答接线测试：deny/allow 档命中时登记即应答（allow 仅
// allow_once），不弹 toast；未命中/仅 allow_always 回落人工询问（红线）。
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole, createMockWsFactory } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { useEndpointMetaStore } = await import("./endpoint-meta");
const { setWsFactory } = await import("./ws-factory");

const EP = { wsUrl: "ws://127.0.0.1:8787", token: "mock-token", peer: "mock-peer", endpointId: "ep-1" };

function responseOutcomeFor(requestId: number): { outcome: string; optionId?: string } | null {
  const hit = mockAcpConsole.responses.find((r) => r.id === requestId);
  const outcome = (hit?.result as { outcome?: { outcome?: string; optionId?: string } })?.outcome;
  return outcome ? { outcome: outcome.outcome ?? "", optionId: outcome.optionId } : null;
}

async function connectActive(): Promise<void> {
  setWsFactory(createMockWsFactory());
  useAcpStore.setState({ draft: { ...EP } });
  useAcpStore.getState().connect();
  await vi.waitFor(() => expect(useAcpStore.getState().phase).toBe("online"));
  await vi.waitFor(() => expect(useAcpStore.getState().activeEndpointId).toBe("ep-1"));
}

async function firePermission(step: {
  toolKind: string;
  title: string;
  options?: unknown[];
}): Promise<number> {
  mockAcpConsole.config.promptScript = [
    { kind: "permission" as const, ...step } as never,
    { kind: "stop", reason: "end_turn" },
  ];
  await useAcpStore.getState().newSession();
  await useAcpStore.getState().sendPrompt("hi");
  // id 基数自 100 起：取末位帧而非按序号查
  const row = mockAcpConsole.permissionRequests[mockAcpConsole.permissionRequests.length - 1];
  expect(row, "权限请求帧未到达").toBeTruthy();
  return row!.id;
}

function interactionOf(requestId: number) {
  const interactions = useAcpStore.getState().interactions;
  for (const inter of Object.values(interactions)) {
    const hit = inter.permissions.find((p) => p.requestId === requestId);
    if (hit) return hit;
  }
  return null;
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  useAcpStore.getState().resetConsoleState();
  useEndpointMetaStore.getState().resetForTest();
});

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
});

describe("endpoint 权限策略自动应答", () => {
  it("deny 档命中：登记即按 reject 选项应答，面板行显策略标记，不弹提醒", async () => {
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { execute: "deny" },
      exceptions: [],
    });
    await connectActive();
    const requestId = await firePermission({ toolKind: "execute", title: "Run rm", options: [
      { optionId: "allow-once", name: "Allow", kind: "allow_once" },
      { optionId: "reject-once", name: "Deny", kind: "reject_once" },
    ] });
    const row = interactionOf(requestId);
    expect(row?.status).toBe("rejected");
    expect(row?.autoAnswered).toBe("deny");
    expect(responseOutcomeFor(requestId)).toEqual({ outcome: "selected", optionId: "reject-once" });
    expect(useAcpStore.getState().permissionNotice).toBeNull();
  });

  it("allow 档命中：仅代答 allow_once，绝不代答 allow_always", async () => {
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { read: "allow" },
      exceptions: [],
    });
    await connectActive();
    const requestId = await firePermission({ toolKind: "read", title: "Read config", options: [
      { optionId: "allow-once", name: "Allow", kind: "allow_once" },
      { optionId: "allow-always", name: "Always", kind: "allow_always" },
    ] });
    const row = interactionOf(requestId);
    expect(row?.status).toBe("approved");
    expect(row?.autoAnswered).toBe("allow");
    expect(responseOutcomeFor(requestId)).toEqual({ outcome: "selected", optionId: "allow-once" });
  });

  it("allow 档但仅 allow_always 可选：回落人工询问（不产生持久放行）", async () => {
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { execute: "allow" },
      exceptions: [],
    });
    await connectActive();
    const requestId = await firePermission({ toolKind: "execute", title: "Deploy", options: [
      { optionId: "allow-always", name: "Always", kind: "allow_always" },
    ] });
    const row = interactionOf(requestId);
    expect(row?.status).toBe("pending");
    expect(row?.autoAnswered).toBeUndefined();
    expect(responseOutcomeFor(requestId)).toBeNull();
    expect(useAcpStore.getState().permissionNotice?.requestId).toBe(requestId);
  });

  it("ask 档（未配置策略）：现行为不变，人工应答", async () => {
    await connectActive();
    const requestId = await firePermission({ toolKind: "execute", title: "Run rm" });
    const row = interactionOf(requestId);
    expect(row?.status).toBe("pending");
    expect(useAcpStore.getState().permissionNotice?.requestId).toBe(requestId);
  });

  it("标题例外规则优先：默认 deny 下精确命中的标题走 allow", async () => {
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { execute: "deny" },
      exceptions: [{ title: "Run tests", tier: "allow" }],
    });
    await connectActive();
    const requestId = await firePermission({ toolKind: "execute", title: "Run tests", options: [
      { optionId: "allow-once", name: "Allow", kind: "allow_once" },
      { optionId: "reject-once", name: "Deny", kind: "reject_once" },
    ] });
    expect(interactionOf(requestId)?.status).toBe("approved");
    expect(responseOutcomeFor(requestId)?.optionId).toBe("allow-once");
  });
});
