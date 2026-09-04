// WS 帧到 ndjson 行的解码与重组。
// 真机对拍（docs/notes/2026-09-05-acp-real-calibration.md R3i）：console 泵按
// 64KiB 块把 P2P 字节流转成 Binary 帧——一帧可含多行、一行可跨多帧，帧载荷
// 随 binaryType 呈现为 ArrayBuffer/Blob（默认 blob 下 String() 会毁帧）。故
// GUI 必须解码帧载荷并按行界重组，禁止按"一帧 = 一条 JSON"整帧 parse。

function isBlob(data: unknown): data is Blob {
  return typeof Blob !== "undefined" && data instanceof Blob;
}

function isArrayBuffer(data: unknown): data is ArrayBuffer {
  // 跨 realm（jsdom 测试环境 vs Node 全局）instanceof 不可靠，用内部 class 串兜底
  return (
    data instanceof ArrayBuffer || Object.prototype.toString.call(data) === "[object ArrayBuffer]"
  );
}

export async function decodeFrame(data: unknown): Promise<string> {
  if (typeof data === "string") return data;
  if (isArrayBuffer(data)) return new TextDecoder().decode(data);
  if (ArrayBuffer.isView(data)) return new TextDecoder().decode(data as Uint8Array);
  if (isBlob(data)) return await data.text();
  console.warn("[acp] 未识别的 WS 帧载荷类型，已丢弃", typeof data);
  return "";
}

/** 增量喂帧文本，吐完整行（行界 = "\n"；残行留缓冲等下一帧） */
export class NdjsonAssembler {
  private buffer = "";

  push(text: string): string[] {
    if (text.length === 0) return [];
    this.buffer += text;
    const lines: string[] = [];
    let newlineAt = this.buffer.indexOf("\n");
    while (newlineAt >= 0) {
      lines.push(this.buffer.slice(0, newlineAt));
      this.buffer = this.buffer.slice(newlineAt + 1);
      newlineAt = this.buffer.indexOf("\n");
    }
    return lines;
  }
}
