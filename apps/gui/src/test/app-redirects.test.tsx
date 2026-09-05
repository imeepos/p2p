import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

// 5.3 旧路由重定向表的机械验收（每行一条断言）：整应用真实挂载后驱动
// hash 导航，断言常驻重定向层落点与 query 透传。
// 与 app-boot.test.tsx 同一机制：先 stub 再动态 import（ipc.ts 求值期读值），
// vitest 按文件隔离模块注册表，双 boot 互不干扰。
vi.stubEnv("VITE_MOCK_IPC", "1");

const BOOT_TIMEOUT = 30_000;
const REDIRECT_WAIT_TIMEOUT = 10_000;

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

function waitForHash(expected: string): Promise<void> {
  return vi.waitFor(
    () => {
      expect(window.location.hash).toBe(expected);
    },
    { timeout: REDIRECT_WAIT_TIMEOUT },
  );
}

describe.each([
  ["/", "/network/overview"],
  ["/peers", "/network/peers"],
  ["/discovery", "/network/discovery"],
  ["/relay", "/network/relay"],
  ["/events", "/network/events"],
  ["/diagnostics", "/network/diagnostics"],
  ["/group", "/chat?kind=group"],
  ["/acp", "/chat?kind=agent"],
  ["/network", "/network/overview"],
])("5.3 重定向行 %s", (from, to) => {
  it(`重定向到 ${to}`, async () => {
    window.location.hash = "#" + from;
    await waitForHash("#" + to);
  });
});

describe("5.3 query 透传与路由保留", () => {
  it("旧路由带参不丢参：/peers?tab=1 落 /network/peers?tab=1", async () => {
    window.location.hash = "#/peers?tab=1";
    await waitForHash("#/network/peers?tab=1");
  });

  it("带参深链合并：/group?x=1 落 /chat?kind=group&x=1（目标参数优先）", async () => {
    window.location.hash = "#/group?x=1";
    await waitForHash("#/chat?kind=group&x=1");
  });

  it("/chat?peer=x 无重定向且参数原样保留", async () => {
    window.location.hash = "#/chat?peer=x";
    await waitForHash("#/chat?peer=x");
  });

  it("/settings 路由保留无重定向", async () => {
    window.location.hash = "#/settings";
    await waitForHash("#/settings");
  });
});
