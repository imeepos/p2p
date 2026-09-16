import { beforeEach, describe, expect, it } from "vitest";

import { createMockServices } from "@/lib/mock-services";

// mock-services 冒烟：闭集清单/双读回落/闭集外报错/损坏拒读与 §20 契约一致，
// E2E 与后续波复用本实现时以此为语义锚。

const STOPPED_DEPS = {
  isRunning: () => false,
  enableMdns: () => true,
  lanOnly: () => false,
};

function createHarness(overrides?: Partial<typeof STOPPED_DEPS>) {
  return createMockServices({ ...STOPPED_DEPS, ...overrides });
}

const mock = createHarness();
const mockBackend = mock.backend;
const controller = mock.controller;

beforeEach(() => {
  controller.reset();
});

describe("mock servicesList", () => {
  it("返回闭集 11 项且 id/kind 与 §20.1 一致", async () => {
    const { services } = await mockBackend.servicesList();
    expect(services).toHaveLength(11);
    expect(services.map((s) => [s.serviceId, s.kind])).toEqual([
      ["serve.llm_share", "boolean"],
      ["serve.tunnel", "boolean"],
      ["serve.a2a", "explicit"],
      ["serve.acp", "explicit"],
      ["net.rendezvous_register", "explicit"],
      ["net.relay", "explicit"],
      ["net.observe", "explicit"],
      ["serve.rendezvous_server", "explicit"],
      ["discovery.mdns", "adopted"],
      ["net.lan_only", "adopted"],
      ["serve.ftp", "boolean"],
    ]);
  });

  it("无条目时按默认与双读回落推导 enabled", async () => {
    const { services } = await mockBackend.servicesList();
    const byId = new Map(services.map((s) => [s.serviceId, s.enabled]));
    expect(byId.get("serve.llm_share")).toBe(false);
    expect(byId.get("serve.a2a")).toBe(true);
    expect(byId.get("discovery.mdns")).toBe(true);
    expect(byId.get("net.lan_only")).toBe(false);
  });

  it("节点运行中 requiresRestart=true，未运行为 false", async () => {
    const stopped = await mockBackend.servicesList();
    expect(stopped.services.every((s) => !s.requiresRestart)).toBe(true);
    const running = createHarness({ isRunning: () => true }).backend;
    const started = await running.servicesList();
    expect(started.services.every((s) => s.requiresRestart)).toBe(true);
  });

  it("损坏态拒读且不静默回退空表", async () => {
    controller.setCorrupt(true);
    await expect(mockBackend.servicesList()).rejects.toThrow("services.json 损坏");
  });
});

describe("mock servicesSetEnabled", () => {
  it("upsert 条目并回持久化值与 requiresRestart", async () => {
    const report = await mockBackend.servicesSetEnabled("serve.tunnel", true);
    expect(report).toEqual({
      serviceId: "serve.tunnel",
      enabled: true,
      requiresRestart: false,
    });
    const { services } = await mockBackend.servicesList();
    expect(services.find((s) => s.serviceId === "serve.tunnel")?.enabled).toBe(
      true,
    );
  });

  it("收编型翻转后条目权威，覆盖双读回落", async () => {
    const adopted = createHarness({ enableMdns: () => true, isRunning: () => true });
    await adopted.backend.servicesSetEnabled("discovery.mdns", false);
    const { services } = await adopted.backend.servicesList();
    const mdns = services.find((s) => s.serviceId === "discovery.mdns");
    expect(mdns?.enabled).toBe(false);
    expect(mdns?.requiresRestart).toBe(true);
  });

  it("表外 serviceId 报可读中文并附闭集清单", async () => {
    await expect(
      mockBackend.servicesSetEnabled("serve.nope", true),
    ).rejects.toThrow(/不在服务闭集.*serve\.llm_share/);
  });

  it("损坏态拒绝写入（不得部分生效）", async () => {
    controller.setCorrupt(true);
    await expect(
      mockBackend.servicesSetEnabled("serve.acp", false),
    ).rejects.toThrow("services.json 损坏");
    controller.setCorrupt(false);
    const { services } = await mockBackend.servicesList();
    expect(services.find((s) => s.serviceId === "serve.acp")?.enabled).toBe(
      true,
    );
  });
});
