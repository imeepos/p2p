// rd 帧通道解析（gui-contract §21.4）：[w:u16 LE][h:u16 LE][seq:u32 LE][rgba8]。
// 纯函数，浏览器 mock 与 tauri 通道共用同一二进制格式。

export const RD_FRAME_HEADER_LEN = 8;

export interface RdFrame {
  w: number;
  h: number;
  seq: number;
  rgba: Uint8ClampedArray<ArrayBuffer>;
}

/** 解析一帧；长度不符或超出 u16 尺寸返回 null（不抛错，坏帧丢弃）。 */
export function parseRdFrame(buffer: ArrayBuffer): RdFrame | null {
  if (buffer.byteLength <= RD_FRAME_HEADER_LEN) return null;
  if (buffer.byteLength % 4 !== 0) return null;
  const head = new DataView(buffer);
  const w = head.getUint16(0, true);
  const h = head.getUint16(2, true);
  const seq = head.getUint32(4, true);
  if (w === 0 || h === 0) return null;
  const expected = RD_FRAME_HEADER_LEN + w * h * 4;
  if (buffer.byteLength !== expected) return null;
  return {
    w,
    h,
    seq,
    rgba: new Uint8ClampedArray(buffer, RD_FRAME_HEADER_LEN, w * h * 4),
  };
}

/** 编码一帧（mock 帧泵用，与 Rust encode_frame 同格式）。 */
export function encodeRdFrame(w: number, h: number, seq: number, rgba: Uint8ClampedArray): ArrayBuffer {
  const out = new ArrayBuffer(RD_FRAME_HEADER_LEN + w * h * 4);
  const view = new DataView(out);
  view.setUint16(0, w, true);
  view.setUint16(2, h, true);
  view.setUint32(4, seq, true);
  new Uint8Array(out, RD_FRAME_HEADER_LEN).set(rgba);
  return out;
}
