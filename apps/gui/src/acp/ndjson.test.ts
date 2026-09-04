// ndjson 帧重组单测：对齐真机对拍 R3i（一帧多行合帧 / 一行跨多帧拆分）。
import { describe, expect, it } from "vitest";

import { NdjsonAssembler, decodeFrame } from "./ndjson";

describe("NdjsonAssembler", () => {
  it("单帧单行", () => {
    const asm = new NdjsonAssembler();
    expect(asm.push('{"a":1}\n')).toEqual(['{"a":1}']);
  });

  it("一帧多行（console 64KiB 块合帧实测）", () => {
    const asm = new NdjsonAssembler();
    expect(asm.push('{"a":1}\n{"b":2}\n')).toEqual(['{"a":1}', '{"b":2}']);
  });

  it("残行跨帧：半行先到不产出，补齐后产出", () => {
    const asm = new NdjsonAssembler();
    expect(asm.push('{"a"')).toEqual([]);
    expect(asm.push(":1}\n")).toEqual(['{"a":1}']);
  });

  it("空帧与无行界输入保持缓冲", () => {
    const asm = new NdjsonAssembler();
    expect(asm.push("")).toEqual([]);
    expect(asm.push("{")).toEqual([]);
    expect(asm.push("}\n")).toEqual(["{}"]);
  });
});

describe("decodeFrame", () => {
  it("string 直通", async () => {
    await expect(decodeFrame('{"a":1}\n')).resolves.toBe('{"a":1}\n');
  });

  it("ArrayBuffer 解码", async () => {
    const bytes = new TextEncoder().encode('{"a":1}\n');
    await expect(decodeFrame(bytes.buffer as ArrayBuffer)).resolves.toBe('{"a":1}\n');
  });

  it("Uint8Array 视图解码（带偏移）", async () => {
    const full = new TextEncoder().encode('{"a":1}\n');
    const view = full.subarray(0, full.length);
    await expect(decodeFrame(view)).resolves.toBe('{"a":1}\n');
  });

  it("Blob 解码（binaryType=blob 浏览器默认形态）", async () => {
    await expect(decodeFrame(new Blob(['{"a":1}\n']))).resolves.toBe('{"a":1}\n');
  });

  it("未识别类型回空串（调用方留告警）", async () => {
    await expect(decodeFrame(42)).resolves.toBe("");
  });
});
