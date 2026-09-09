import { fireEvent } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

// 4.1 tab 机械验收：六 tab 渲染、子路由直达、点击切换、会话级记忆与
// 全新会话落 overview。整应用真实挂载（与 app-redirects 同机制：先 stub
// 再动态 import，vitest 按文件隔离模块注册表）。
// S4 追记：高负载下默认并行可能出现文件级错（forks worker 未起，测试体 0ms），
// 测试内等待无法治；验收口径串行 `pnpm test:serial`（docs/design/authz-a3-plan.md §4）。
vi.stubEnv("VITE_MOCK_IPC", "1");

import { resetNetworkTabMemory } from "@/views/network/network-tab-memory";

const BOOT_TIMEOUT = 30_000;
const WAIT_TIMEOUT = 10_000;

let host: HTMLElement;

beforeAll(async () => {
  host = document.createElement("div");
  host.id = "root";
  document.body.appendChild(host);
  await import("../main");
  await vi.waitFor(
    () => {
      expect(host.querySelector("main")).not.toBeNull();
    },
    { timeout: BOOT_TIMEOUT },
  );
}, BOOT_TIMEOUT + 10_000);

afterAll(() => {
  document.body.innerHTML = "";
  window.location.hash = "#/";
});

async function waitForHash(expected: string): Promise<void> {
  await vi.waitFor(
    () => {
      expect(window.location.hash).toBe(expected);
    },
    { timeout: WAIT_TIMEOUT },
  );
}

function tabByName(name: string): HTMLElement {
  const tabs = [...host.querySelectorAll('[role="tab"]')];
  const tab = tabs.find((t) => t.textContent === name);
  expect(tab, "tab " + name + " 应存在").toBeTruthy();
  return tab as HTMLElement;
}

describe("network tab 条与子路由", () => {
  it("六 tab 渲染且子路由直达：/network/relay 选中中继", async () => {
    window.location.hash = "#/network/relay";
    await waitForHash("#/network/relay");
    await vi.waitFor(() => {
      expect(host.textContent).toContain("中继地址配置");
    }, { timeout: WAIT_TIMEOUT });
    const tabs = [...host.querySelectorAll('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual([
      "概览", "节点", "发现", "中继", "事件", "诊断",
    ]);
    for (const tab of tabs) {
      const selected = tab.getAttribute("aria-selected") === "true";
      expect(selected).toBe(tab.textContent === "中继");
    }
  });

  it("点击 tab 切换子路由并更新选中态", async () => {
    fireEvent.click(tabByName("事件"));
    await waitForHash("#/network/events");
    expect(tabByName("事件").getAttribute("aria-selected")).toBe("true");
    expect(tabByName("中继").getAttribute("aria-selected")).toBe("false");
  });

  it("会话级记忆：重进 /network 无子路由落最后停留 tab", async () => {
    window.location.hash = "#/network";
    await waitForHash("#/network/events");
  });

  it("重置记忆（全新会话语义）后 /network 落 overview", async () => {
    window.location.hash = "#/network/diagnostics";
    await waitForHash("#/network/diagnostics");
    resetNetworkTabMemory();
    window.location.hash = "#/network";
    await waitForHash("#/network/overview");
    await vi.waitFor(() => {
      // F22：页头与 tab 同名「概览」
      expect(host.textContent).toContain("概览");
    }, { timeout: WAIT_TIMEOUT });
  });
});
