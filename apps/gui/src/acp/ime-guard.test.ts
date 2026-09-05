// F1 单元测试：IME 组合态判定（isComposing 标准路径 + keyCode 229 兜底）。
import { describe, expect, it } from "vitest";

import { isImeComposing } from "./ime-guard";

describe("isImeComposing", () => {
  it("isComposing=true 判定为组合态", () => {
    expect(isImeComposing({ isComposing: true })).toBe(true);
  });

  it("keyCode=229 兜底：未置 isComposing 的组合态也判定为组合态", () => {
    expect(isImeComposing({ isComposing: false, keyCode: 229 })).toBe(true);
    expect(isImeComposing({ keyCode: 229 })).toBe(true);
  });

  it("普通 Enter（非组合态）不误判", () => {
    expect(isImeComposing({ isComposing: false })).toBe(false);
    expect(isImeComposing({})).toBe(false);
    expect(isImeComposing({ isComposing: false, keyCode: 13 })).toBe(false);
  });
});
