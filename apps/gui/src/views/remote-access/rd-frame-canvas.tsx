import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { parseRdFrame } from "@/lib/rd-frame";
import {
  buttonsOf,
  clampWheel,
  hidCodeOf,
  modifiersOf,
} from "@/lib/rd-keymap";

import type { UseRdPageModel } from "./use-rd-model";

interface Props {
  model: UseRdPageModel;
}

// viewer 画面画布（gui-contract §21.4）：帧通道渲染 + 鼠标/键盘事件下发。
// 帧到达即 putImageData（host fps ≤ 60 无需 rAF 节流）；失焦/卸载发 key_reset 防卡键。
export function RdFrameCanvas({ model }: Props) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dimsRef = useRef<{ w: number; h: number } | null>(null);
  const [aspect, setAspect] = useState<string>("16 / 9");

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let lastSeq = 0;
    const unsubscribe = model.onFrame((buf) => {
      const frame = parseRdFrame(buf);
      if (!frame || frame.seq === lastSeq) return;
      lastSeq = frame.seq;
      if (!dimsRef.current || dimsRef.current.w !== frame.w || dimsRef.current.h !== frame.h) {
        dimsRef.current = { w: frame.w, h: frame.h };
        canvas.width = frame.w;
        canvas.height = frame.h;
        setAspect(`${frame.w} / ${frame.h}`);
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.putImageData(new ImageData(frame.rgba, frame.w, frame.h), 0, 0);
    });
    return unsubscribe;
  }, [model]);

  const toFrameCoords = useCallback((event: { clientX: number; clientY: number }) => {
    const canvas = canvasRef.current;
    const dims = dimsRef.current;
    if (!canvas || !dims) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const x = Math.max(0, Math.min(dims.w - 1, ((event.clientX - rect.left) / rect.width) * dims.w));
    const y = Math.max(0, Math.min(dims.h - 1, ((event.clientY - rect.top) / rect.height) * dims.h));
    return { x: Math.round(x), y: Math.round(y) };
  }, []);

  const handlePointer = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const pos = toFrameCoords(event);
      if (!pos) return;
      void model.sendMouse(pos.x, pos.y, buttonsOf(event), 0, 0);
    },
    [model, toFrameCoords],
  );

  const handleWheel = useCallback(
    (event: React.WheelEvent<HTMLCanvasElement>) => {
      const pos = toFrameCoords(event);
      if (!pos) return;
      event.preventDefault();
      void model.sendMouse(pos.x, pos.y, buttonsOf(event), clampWheel(event.deltaX), clampWheel(event.deltaY));
    },
    [model, toFrameCoords],
  );

  const handleKey = useCallback(
    (event: React.KeyboardEvent<HTMLCanvasElement>, down: boolean) => {
      const code = hidCodeOf(event.code);
      if (code === null) return;
      event.preventDefault();
      void model.sendKey(code, down, modifiersOf(event));
    },
    [model],
  );

  useEffect(() => {
    // 卸载兜底：防画布销毁后 host 侧按键滞留。
    return () => {
      void model.resetKeys();
    };
  }, [model]);

  return (
    <div className="space-y-1">
      <canvas
        ref={canvasRef}
        data-testid="rd-frame-canvas"
        tabIndex={0}
        style={{ aspectRatio: aspect }}
        className="w-full cursor-crosshair rounded border bg-black outline-none focus-visible:ring-1 focus-visible:ring-ring"
        onPointerDown={handlePointer}
        onPointerUp={handlePointer}
        onPointerMove={handlePointer}
        onWheel={handleWheel}
        onKeyDown={(e) => handleKey(e, true)}
        onKeyUp={(e) => handleKey(e, false)}
        onBlur={() => void model.resetKeys()}
      />
      <div className="text-xs text-muted-foreground">{t("remoteAccess.rd.viewer.canvasHint")}</div>
    </div>
  );
}
