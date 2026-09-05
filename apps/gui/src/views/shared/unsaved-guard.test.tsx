import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, Link, RouterProvider } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import { UnsavedRouteGuard, useUnsavedGuard } from "./unsaved-guard";

// 代表性脏编辑面：hasUnsaved/discard 由外部可变旗标驱动，覆盖注册表全链路。
let dirty = false;
let discardCount = 0;

function GuardedPage({ label, target }: { label: string; target: string }) {
  useUnsavedGuard("guard-test-" + label, {
    hasUnsaved: () => dirty,
    discard: () => {
      discardCount += 1;
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
