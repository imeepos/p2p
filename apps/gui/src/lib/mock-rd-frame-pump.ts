import { encodeRdFrame } from "./rd-frame";

// mock viewer 帧泵：确定性图案（与 Rust SyntheticSource 同思路的简化版），
// 供浏览器 dev 与渲染矩阵测试使用；固定 320×180、10fps（dev 观感优先）。

const MOCK_W = 320;
const MOCK_H = 180;
const MOCK_INTERVAL_MS = 100;

export interface MockFramePump {
  stop(): void;
  framesSent(): number;
}

/** 启动帧泵：每 tick 产出一帧经 onFrame 投递；stop 幂等。 */
export function startFramePump(onFrame: (buf: ArrayBuffer) => void): MockFramePump {
  let seq = 0;
  let stopped = false;
  const rgba = new Uint8ClampedArray(MOCK_W * MOCK_H * 4);
  const timer = window.setInterval(() => {
    if (stopped) return;
    seq += 1;
    for (let y = 0; y < MOCK_H; y += 1) {
      for (let x = 0; x < MOCK_W; x += 1) {
        const i = (y * MOCK_W + x) * 4;
        rgba[i] = (x + seq * 3) & 0xff;
        rgba[i + 1] = (y + seq * 5) & 0xff;
        rgba[i + 2] = (x + y) & 0xff;
        rgba[i + 3] = 255;
      }
    }
    onFrame(encodeRdFrame(MOCK_W, MOCK_H, seq, rgba));
  }, MOCK_INTERVAL_MS);
  return {
    stop() {
      stopped = true;
      window.clearInterval(timer);
    },
    framesSent: () => seq,
  };
}

export const MOCK_FRAME_DIMS = { w: MOCK_W, h: MOCK_H };
