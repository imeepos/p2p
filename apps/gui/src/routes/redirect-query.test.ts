import { describe, expect, it } from "vitest";

import { mergeRedirectQuery } from "./redirect-query";

describe("mergeRedirectQuery（5.3 统一透传函数）", () => {
  it("无 query 时返回目标原样", () => {
    expect(mergeRedirectQuery("/network/peers", "")).toBe("/network/peers");
    expect(mergeRedirectQuery("/network/overview", "?")).toBe("/network/overview");
  });

  it("旧路由 query 原样透传", () => {
    expect(mergeRedirectQuery("/network/peers", "?tab=1")).toBe(
      "/network/peers?tab=1",
    );
  });

  it("目标既有参数优先，旧 query 并入其后", () => {
    expect(mergeRedirectQuery("/chat?kind=group", "?x=1")).toBe(
      "/chat?kind=group&x=1",
    );
  });

  it("同名参数以重定向目标为准，不重复透传", () => {
    expect(mergeRedirectQuery("/chat?kind=group", "?kind=agent")).toBe(
      "/chat?kind=group",
    );
  });

  it("多参数保持出现顺序", () => {
    expect(mergeRedirectQuery("/chat?kind=agent", "?a=1&b=2")).toBe(
      "/chat?kind=agent&a=1&b=2",
    );
  });
});
