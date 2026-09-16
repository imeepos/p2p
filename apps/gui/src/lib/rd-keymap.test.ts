import { describe, expect, it } from "vitest";

import {
  buttonsOf,
  clampWheel,
  hidCodeOf,
  modifiersOf,
} from "./rd-keymap";

// DOM 键鼠 → rd 线协议编码（rd-control.md：bit0 左/bit1 右/bit2 中；
// modifiers 0x1 shift/0x2 ctrl/0x4 alt/0x8 meta；键码 USB HID usage id）。
describe("rd-keymap", () => {
  it("modifiers 位掩码", () => {
    expect(modifiersOf({ shiftKey: false, ctrlKey: false, altKey: false, metaKey: false })).toBe(0);
    expect(modifiersOf({ shiftKey: true, ctrlKey: false, altKey: false, metaKey: false })).toBe(0x1);
    expect(modifiersOf({ shiftKey: false, ctrlKey: true, altKey: true, metaKey: false })).toBe(0x6);
    expect(modifiersOf({ shiftKey: false, ctrlKey: false, altKey: false, metaKey: true })).toBe(0x8);
  });

  it("buttons 位序转换为协议位（DOM 右=2 中=4 → 协议 右=bit1 中=bit2）", () => {
    expect(buttonsOf({ buttons: 1 })).toBe(0x1);
    expect(buttonsOf({ buttons: 2 })).toBe(0x4);
    expect(buttonsOf({ buttons: 4 })).toBe(0x2);
    expect(buttonsOf({ buttons: 7 })).toBe(0x7);
  });

  it("HID 键码：字母/数字/功能键/特殊键", () => {
    expect(hidCodeOf("KeyA")).toBe(0x04);
    expect(hidCodeOf("KeyZ")).toBe(0x1d);
    expect(hidCodeOf("Digit1")).toBe(0x1e);
    expect(hidCodeOf("Digit0")).toBe(0x27);
    expect(hidCodeOf("F1")).toBe(0x3a);
    expect(hidCodeOf("F12")).toBe(0x45);
    expect(hidCodeOf("Space")).toBe(0x2c);
    expect(hidCodeOf("ArrowUp")).toBe(0x52);
    expect(hidCodeOf("ControlLeft")).toBe(0xe0);
    expect(hidCodeOf("NotAKey")).toBeNull();
  });

  it("滚轮增量收敛到 i8", () => {
    expect(clampWheel(3.7)).toBe(3);
    expect(clampWheel(-1000)).toBe(-128);
    expect(clampWheel(1000)).toBe(127);
    expect(clampWheel(Number.NaN)).toBe(0);
  });
});
