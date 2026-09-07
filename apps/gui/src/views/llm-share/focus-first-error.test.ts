import { describe, expect, it, vi } from "vitest";

import { focusFirstInvalidField } from "./focus-first-error";

describe("R2-12 focusFirstInvalidField", () => {
  it("聚焦第一个无效字段并返回错误字段数", () => {
    HTMLElement.prototype.scrollIntoView = vi.fn();
    document.body.innerHTML =
      '<input id="a"/><input id="b"/><input id="c"/>';
    const count = focusFirstInvalidField(["a", "b", "c"], (id) => id !== "a");
    expect(count).toBe(2);
    expect(document.activeElement?.id).toBe("b");
  });

  it("全部有效时不聚焦返回 0；节点缺失时留 console 信号不静默", () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    HTMLElement.prototype.scrollIntoView = vi.fn();
    document.body.innerHTML = '<input id="a"/>';
    expect(focusFirstInvalidField(["a"], () => false)).toBe(0);
    expect(focusFirstInvalidField(["missing"], () => true)).toBe(1);
    expect(errSpy).toHaveBeenCalled();
    errSpy.mockRestore();
  });
});
