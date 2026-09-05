import { beforeEach, describe, expect, it, vi } from "vitest";

import { policyOf, useEndpointMetaStore } from "./endpoint-meta";

const META_KEY = "p2p-gui-acp-endpoint-meta";

describe("endpoint-meta store", () => {
  beforeEach(() => {
    localStorage.clear();
    useEndpointMetaStore.getState().resetForTest();
  });

  it("停用位与测试结论写穿 localStorage", () => {
    useEndpointMetaStore.getState().setDisabled("ep-1", true);
    useEndpointMetaStore.getState().recordTest("ep-1", "failed");
    const raw = JSON.parse(localStorage.getItem(META_KEY) ?? "{}");
    expect(raw.disabled["ep-1"]).toBe(true);
    expect(raw.lastTest["ep-1"]).toBe("failed");
  });

  it("策略写入可读回；policyOf 未配置兜底空策略（全 ask）", () => {
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { execute: "deny" },
      exceptions: [{ title: "Run tests", tier: "allow" }],
    });
    expect(useEndpointMetaStore.getState().policies["ep-1"]?.defaults.execute).toBe("deny");
    expect(policyOf("ep-1")!.defaults.execute).toBe("deny");
    expect(policyOf("ep-unknown")).toEqual({ defaults: {}, exceptions: [] });
    expect(policyOf(null)).toEqual({ defaults: {}, exceptions: [] });
  });

  it("forget 清空该 endpoint 全部元数据（删除 endpoint 防孤儿键）", () => {
    useEndpointMetaStore.getState().setDisabled("ep-1", true);
    useEndpointMetaStore.getState().recordTest("ep-1", "ok");
    useEndpointMetaStore.getState().setPolicy("ep-1", { defaults: { read: "allow" }, exceptions: [] });
    useEndpointMetaStore.getState().forget("ep-1");
    const state = useEndpointMetaStore.getState();
    expect(state.disabled["ep-1"]).toBeUndefined();
    expect(state.lastTest["ep-1"]).toBeUndefined();
    expect(state.policies["ep-1"]).toBeUndefined();
  });

  it("存档损坏：显式告警并回空白档，不静默", async () => {
    localStorage.setItem(META_KEY, "{not-json");
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // 模块级单例：损坏存档须在全新 import（重 hydrate）时被告警并回空白
    vi.resetModules();
    const fresh = await import("./endpoint-meta");
    expect(fresh.useEndpointMetaStore.getState().policies).toEqual({});
    expect(fresh.useEndpointMetaStore.getState().disabled).toEqual({});
    expect(warnSpy).toHaveBeenCalledWith("[acp] endpoint 元数据存档不可读，使用空白档", expect.anything());
    warnSpy.mockRestore();
  });

  it("resetForTest 清空并落盘空白档", () => {
    useEndpointMetaStore.getState().setDisabled("ep-1", true);
    useEndpointMetaStore.getState().resetForTest();
    expect(useEndpointMetaStore.getState().disabled).toEqual({});
    const raw = JSON.parse(localStorage.getItem(META_KEY) ?? "{}");
    expect(raw.disabled).toEqual({});
  });
});
