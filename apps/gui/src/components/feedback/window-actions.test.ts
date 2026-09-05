import { describe, expect, it } from "vitest";

import { goToHomeRoute } from "./window-actions";

describe("window-actions", () => {
  it("goToHomeRoute 复位 hash 到首页（HashRouter 契约，Router 无关）", () => {
    window.location.hash = "#/settings";
    goToHomeRoute();
    expect(window.location.hash).toBe("#/");
  });
});
