// 首用零手填：autofillDraft 单测——只填空白字段、complete 草稿零 IPC、失败面告警留痕。
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AcpConsoleStatus, AcpLocalDescriptor } from "@/lib/ipc-types";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { autofillDraft, changedFields } = await import("./draft-autofill");
const { mockBackend } = await import("@/lib/mock-ipc");

const CONNECTED: AcpConsoleStatus = {
  phase: "connected",
  wsUrl: "ws://127.0.0.1:9999",
  token: "tok-9",
  statusUrl: "http://127.0.0.1:9998",
};

const DESCRIPTOR: AcpLocalDescriptor = {
  adminUrl: "http://127.0.0.1:8123",
  token: "desc-tok",
  peer: "peer-d",
  agentName: "home-agent",
  writtenAtUnix: 1_725_700_000,
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("autofillDraft", () => {
  it("完整草稿零探测：不发任何 IPC，原对象直通", async () => {
    const statusSpy = vi.spyOn(mockBackend, "acpConsoleStatus");
    const descSpy = vi.spyOn(mockBackend, "acpLocalDescriptor");
    const draft = { wsUrl: "ws://x", token: "t", peer: "p" };
    const out = await autofillDraft(draft);
    expect(out.changed).toBe(false);
    expect(out.next).toBe(draft);
    expect(statusSpy).not.toHaveBeenCalled();
    expect(descSpy).not.toHaveBeenCalled();
  });

  it("缺 token/peer：console 快照 + 自描述自动补齐，只填空白字段", async () => {
    vi.spyOn(mockBackend, "acpConsoleStatus").mockResolvedValue(CONNECTED);
    vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue(DESCRIPTOR);
    const out = await autofillDraft({ wsUrl: "ws://keep", token: "", peer: "" });
    expect(out.changed).toBe(true);
    expect(out.next).toEqual({
      wsUrl: "ws://keep",
      token: "tok-9",
      peer: "peer-d",
      statusUrl: "http://127.0.0.1:9998",
    });
  });

  it("status 未 connected / 描述缺失：不臆造字段", async () => {
    vi.spyOn(mockBackend, "acpConsoleStatus").mockResolvedValue({ phase: "connecting" });
    vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue(null);
    const out = await autofillDraft({ wsUrl: "ws://x", token: "", peer: "" });
    expect(out.changed).toBe(false);
    expect(out.next.token).toBe("");
    expect(out.next.peer).toBe("");
  });

  it("IPC 面异常：告警留痕不静默，原样返回", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(mockBackend, "acpConsoleStatus").mockRejectedValue(new Error("down"));
    vi.spyOn(mockBackend, "acpLocalDescriptor").mockRejectedValue(new Error("down"));
    const out = await autofillDraft({ wsUrl: "ws://x", token: "", peer: "" });
    expect(out.changed).toBe(false);
    const autofillWarns = warnSpy.mock.calls.filter((c) => String(c[0]).includes("自动补全"));
    expect(autofillWarns).toHaveLength(2);
  });
});

describe("changedFields", () => {
  it("只回写变化的字段，statusUrl 空↔缺失视为同值", () => {
    expect(
      changedFields(
        { wsUrl: "a", token: "", peer: "" },
        { wsUrl: "a", token: "t", peer: "p", statusUrl: "http://s" },
      ),
    ).toEqual({ token: "t", peer: "p", statusUrl: "http://s" });
    expect(
      changedFields({ wsUrl: "a", token: "t", peer: "p" }, { wsUrl: "a", token: "t", peer: "p" }),
    ).toEqual({});
  });
});
