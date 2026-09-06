import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { isImeComposing, useImeCompositionGuard } from "./use-ime-composition";

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

describe("useImeCompositionGuard", () => {
  it("初始非组合态：普通 Enter 不拦截", () => {
    const { result } = renderHook(() => useImeCompositionGuard());
    expect(result.current.shouldBlockEnter({})).toBe(false);
  });

  it("compositionstart 至 compositionend 之间拦截 Enter", () => {
    const { result } = renderHook(() => useImeCompositionGuard());
    act(() => result.current.compositionHandlers.onCompositionStart());
    // 组合事件进行中：即使事件未携带 isComposing/229 也拦截
    expect(result.current.shouldBlockEnter({ keyCode: 13 })).toBe(true);
    act(() => result.current.compositionHandlers.onCompositionEnd());
    expect(result.current.shouldBlockEnter({ keyCode: 13 })).toBe(false);
  });

  it("组合键码路径：未挂组合事件也按 229/isComposing 拦截", () => {
    const { result } = renderHook(() => useImeCompositionGuard());
    expect(result.current.shouldBlockEnter({ keyCode: 229 })).toBe(true);
    expect(result.current.shouldBlockEnter({ isComposing: true })).toBe(true);
  });
});
