import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, Link, RouterProvider } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import { UnsavedRouteGuard } from "./unsaved-guard";
import { useUnsavedGuard } from "./use-unsaved-guard";

// 代表性脏编辑面：hasUnsaved/discard 由外部可变旗标驱动，覆盖注册表全链路。
let dirty = false;
let discardCount = 0;

function GuardedPage({ label, target }: { label: string; target: string }) {
  useUnsavedGuard("guard-test-" + label, {
    hasUnsaved: () => dirty,
    discard: () => {
      discardCount += 1;
      // 真实 discard 语义：丢草稿后不再脏（否则放行导航会被 blocker 合法拦下）
      dirty = false;
    },
  });
  return (
    <div>
      <span>page-{label}</span>
      {/* 侧栏路径的代表：Link 点击导航 */}
      <Link to={target}>goto{target}</Link>
    </div>
  );
}

function buildRouter(initial = "/a") {
  return createMemoryRouter(
    [
      {
        path: "/a",
        element: (
          <UnsavedRouteGuard>
            <GuardedPage label="a" target="/b" />
          </UnsavedRouteGuard>
        ),
      },
      { path: "/b", element: <span>page-b</span> },
    ],
    { initialEntries: [initial] },
  );
}

function renderGuarded(initial = "/a") {
  const router = buildRouter(initial);
  render(
    <ConfirmProvider>
      <RouterProvider router={router} />
    </ConfirmProvider>,
  );
  return router;
}

beforeEach(() => {
  dirty = false;
  discardCount = 0;
  window.history.replaceState({ idx: 5 }, "", "#/a");
});

afterEach(() => {
  window.history.replaceState(null, "", window.location.pathname);
});

// 模拟直改 hash 的 POP：目标 URL 用 replaceState 就位（零事件），再手动派发
// 缺 idx 的 popstate——不用 location.hash= 赋值，jsdom 对其会异步补发自己的
// popstate（state=null），跨用例串扰。（memory router 不经 window.history，
// 兜底层读的是 jsdom 真实 history）
function dispatchManualHashPop(target: string): void {
  window.history.replaceState(null, "", target);
  window.dispatchEvent(new PopStateEvent("popstate", { state: null }));
}

function dispatchIdxKnownPop(target: string, fromIdx: number, toIdx: number): void {
  window.history.replaceState({ idx: fromIdx }, "", target);
  window.dispatchEvent(new PopStateEvent("popstate", { state: { idx: toIdx } }));
}

describe("UnsavedRouteGuard POP 直改 hash 兜底（F05）", () => {
  it("脏状态手动改 hash 的返回被拦：弹确认，取消则驻留且 URL 回滚", async () => {
    dirty = true;
    renderGuarded();
    dispatchManualHashPop("#/b");
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("留在本页"));
    await waitFor(() =>
      expect(screen.queryByText("放弃未保存的修改？")).toBeNull(),
    );
    expect(screen.getByText("page-a")).toBeTruthy();
    expect(discardCount).toBe(0);
    expect(window.location.hash).toBe("#/a");
  });

  it("脏状态手动改 hash 被拦：放弃则丢草稿并落到目标路由", async () => {
    dirty = true;
    renderGuarded();
    dispatchManualHashPop("#/b");
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("放弃修改"));
    await waitFor(() => expect(discardCount).toBe(1), { timeout: 4000 });
    await waitFor(
      () => expect(screen.getByText("page-b")).toBeTruthy(),
      { timeout: 4000 },
    );
  });

  it("无脏时手动改 hash 的 POP 畅通：不弹确认不丢数据", async () => {
    renderGuarded();
    dispatchManualHashPop("#/b");
    await act(async () => {});
    expect(screen.queryByText("放弃未保存的修改？")).toBeNull();
    expect(discardCount).toBe(0);
  });

  it("delta 可判的 POP 不走兜底（由 router blocker 拦截），不双弹窗", async () => {
    dirty = true;
    renderGuarded();
    dispatchIdxKnownPop("#/b", 5, 4);
    await act(async () => {});
    expect(screen.queryByText("放弃未保存的修改？")).toBeNull();
    expect(discardCount).toBe(0);
  });
});

describe("UnsavedRouteGuard", () => {
  it("不脏时点击链接直接切换，不弹确认", async () => {
    renderGuarded();
    fireEvent.click(screen.getByText("goto/b"));
    await waitFor(() => expect(screen.getByText("page-b")).toBeTruthy());
    expect(screen.queryByText("放弃未保存的修改？")).toBeNull();
  });

  it("脏状态点链接（侧栏路径）弹确认：取消驻留原页且草稿保留", async () => {
    dirty = true;
    renderGuarded();
    fireEvent.click(screen.getByText("goto/b"));
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("留在本页"));
    await waitFor(() =>
      expect(screen.queryByText("放弃未保存的修改？")).toBeNull(),
    );
    expect(screen.getByText("page-a")).toBeTruthy();
    expect(screen.queryByText("page-b")).toBeNull();
    expect(discardCount).toBe(0);
  });

  it("脏状态点链接弹确认：放弃则丢弃草稿并放行", async () => {
    dirty = true;
    renderGuarded();
    fireEvent.click(screen.getByText("goto/b"));
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("放弃修改"));
    await waitFor(() => expect(screen.getByText("page-b")).toBeTruthy());
    expect(discardCount).toBe(1);
  });

  it("脏状态编程导航（快捷键路径 router.navigate）同样被拦截，取消驻留", async () => {
    dirty = true;
    const router = renderGuarded();
    await act(async () => {
      await router.navigate("/b");
    });
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("留在本页"));
    await waitFor(() => expect(screen.getByText("page-a")).toBeTruthy());
    expect(router.state.location.pathname).toBe("/a");
    expect(discardCount).toBe(0);
  });

  it("脏状态编程导航被拦截：放弃清空后落到目标路由", async () => {
    dirty = true;
    const router = renderGuarded();
    await act(async () => {
      await router.navigate("/b");
    });
    expect(await screen.findByText("放弃未保存的修改？")).toBeTruthy();
    fireEvent.click(screen.getByText("放弃修改"));
    await waitFor(() => expect(screen.getByText("page-b")).toBeTruthy());
    expect(router.state.location.pathname).toBe("/b");
    expect(discardCount).toBe(1);
  });
});
