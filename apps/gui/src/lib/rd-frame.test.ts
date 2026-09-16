import { describe, expect, it } from "vitest";

import { encodeRdFrame, parseRdFrame, RD_FRAME_HEADER_LEN } from "./rd-frame";

// 帧编解码往返（gui-contract §21.4 二进制格式）。
describe("rd-frame", () => {
  it("encode→parse 往返保留 w/h/seq/rgba", () => {
    const rgba = new Uint8ClampedArray([1, 2, 3, 255, 5, 6, 7, 255]);
    const buf = encodeRdFrame(2, 1, 42, rgba);
    const frame = parseRdFrame(buf);
    expect(frame).not.toBeNull();
    expect(frame!.w).toBe(2);
    expect(frame!.h).toBe(1);
    expect(frame!.seq).toBe(42);
    expect(Array.from(frame!.rgba)).toEqual([1, 2, 3, 255, 5, 6, 7, 255]);
  });

  it("解析小端帧头（与 Rust encode_frame 对齐）", () => {
    const buf = encodeRdFrame(640, 360, 7, new Uint8ClampedArray(640 * 360 * 4));
    const view = new DataView(buf);
    expect(view.getUint16(0, true)).toBe(640);
    expect(view.getUint16(2, true)).toBe(360);
    expect(view.getUint32(4, true)).toBe(7);
    expect(buf.byteLength).toBe(RD_FRAME_HEADER_LEN + 640 * 360 * 4);
  });

  it("坏帧拒绝：过短/长度不符/零尺寸", () => {
    expect(parseRdFrame(new ArrayBuffer(4))).toBeNull();
    // 头声明 2x1 但载荷缺 4 字节
    const bad = encodeRdFrame(2, 1, 1, new Uint8ClampedArray(2 * 1 * 4));
    expect(parseRdFrame(bad.slice(0, bad.byteLength - 4))).toBeNull();
    // 手工拼零宽帧头（encodeRdFrame 的入参约定要求 w/h 与 rgba 一致）
    const zero = new ArrayBuffer(RD_FRAME_HEADER_LEN + 4);
    const view = new DataView(zero);
    view.setUint16(0, 0, true);
    view.setUint16(2, 1, true);
    view.setUint32(4, 1, true);
    expect(parseRdFrame(zero)).toBeNull();
  });
});
