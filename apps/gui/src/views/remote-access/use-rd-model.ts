import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";
import type { RdFrameListener, RdHostStatus, RdViewerStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";

const IDLE_HOST: RdHostStatus = {
  running: false,
  requireApproval: true,
  sessionCount: 0,
  pendingApprovals: [],
  fps: 15,
};
const IDLE_VIEWER: RdViewerStatus = { connected: false, sessionId: null };

// 远程桌面页状态模型（gui-contract §21）：host/审批/质量/viewer 命令面快照同步。
// 节点未运行一律显式告警（不静默）；动作期 busy 防重入。
export function useRdPageModel() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running === true);
  const [hostRaw, setHost] = useState<RdHostStatus>(IDLE_HOST);
  const [viewerRaw, setViewer] = useState<RdViewerStatus>(IDLE_VIEWER);
  const [busy, setBusy] = useState<"start" | "stop" | "connect" | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);
  // 帧订阅集（§21.4）：connect 时把派发器挂进通道，组件经 onFrame 订阅。
  const frameListeners = useRef(new Set<RdFrameListener>());

  // 节点未运行：派生回退空态（避免 effect 内 setState，react-hooks 规则）。
  const host = running ? hostRaw : IDLE_HOST;
  const viewer = running ? viewerRaw : IDLE_VIEWER;

  useEffect(() => {
    if (!running) return;
    let alive = true;
    void (async () => {
      try {
        const hostNext = await ipc.rdHostStatus();
        const viewerNext = await ipc.rdViewerStatus();
        if (!alive) return;
        setHost(hostNext);
        setViewer(viewerNext);
      } catch (error) {
        console.error("[rd] 状态读取失败", error);
      }
    })();
    return () => {
      alive = false;
    };
  }, [running]);

  const guardRunning = useCallback((): boolean => {
    if (!running) {
      const message = t("remoteAccess.rd.errors.nodeOffline");
      setLastError(message);
      return false;
    }
    return true;
  }, [running, t]);

  const startHost = useCallback(
    async (requireApproval: boolean) => {
      if (!guardRunning()) return;
      setBusy("start");
      setLastError(null);
      try {
        setHost(await ipc.rdHostStart(requireApproval));
      } catch (error) {
        const message = String(error);
        setLastError(message);
        toastError(message);
      } finally {
        setBusy(null);
      }
    },
    [guardRunning],
  );

  const stopHost = useCallback(async () => {
    if (!guardRunning()) return;
    setBusy("stop");
    setLastError(null);
    try {
      setHost(await ipc.rdHostStop());
      setViewer(IDLE_VIEWER);
    } catch (error) {
      const message = String(error);
      setLastError(message);
      toastError(message);
    } finally {
      setBusy(null);
    }
  }, [guardRunning]);

  const approve = useCallback(
    async (peer: string) => {
      if (!guardRunning()) return;
      try {
        await ipc.rdApprove(peer);
        setHost(await ipc.rdHostStatus());
      } catch (error) {
        toastError(String(error));
      }
    },
    [guardRunning],
  );

  const deny = useCallback(
    async (peer: string) => {
      if (!guardRunning()) return;
      try {
        await ipc.rdDeny(peer);
        setHost(await ipc.rdHostStatus());
      } catch (error) {
        toastError(String(error));
      }
    },
    [guardRunning],
  );

  const setQuality = useCallback(
    async (fps: number) => {
      if (!host.running) return;
      try {
        setHost(await ipc.rdQualitySet(fps, 100, 0));
      } catch (error) {
        toastError(String(error));
      }
    },
    [host.running],
  );

  const connectViewer = useCallback(
    async (peer: string) => {
      if (!guardRunning()) return;
      setBusy("connect");
      setLastError(null);
      try {
        const dispatch: RdFrameListener = (buf) => {
          frameListeners.current.forEach((listener) => listener(buf));
        };
        setViewer(await ipc.rdViewerConnect(peer, undefined, dispatch));
      } catch (error) {
        const message = String(error);
        setLastError(message);
        toastError(message);
      } finally {
        setBusy(null);
      }
    },
    [guardRunning],
  );

  // 帧订阅（canvas 渲染入口）；返回退订函数。
  const onFrame = useCallback((listener: RdFrameListener) => {
    frameListeners.current.add(listener);
    return () => {
      frameListeners.current.delete(listener);
    };
  }, []);

  // 输入发送（§21.4）：断连时静默 false（UI 已呈未连接态）；
  // 真发送失败 warn 留观测，不 toast 轰炸高频事件。
  const sendMouse = useCallback(
    async (x: number, y: number, buttons: number, wheelDx: number, wheelDy: number) => {
      if (!viewer.connected) return false;
      try {
        return await ipc.rdInputMouse(x, y, buttons, wheelDx, wheelDy);
      } catch (error) {
        console.warn("[rd] 鼠标事件下发失败", error);
        return false;
      }
    },
    [viewer.connected],
  );

  const sendKey = useCallback(
    async (code: number, down: boolean, modifiers: number) => {
      if (!viewer.connected) return false;
      try {
        return await ipc.rdInputKey(code, down, modifiers);
      } catch (error) {
        console.warn("[rd] 键盘事件下发失败", error);
        return false;
      }
    },
    [viewer.connected],
  );

  const resetKeys = useCallback(async () => {
    if (!viewer.connected) return false;
    try {
      return await ipc.rdInputKeyReset();
    } catch (error) {
      console.warn("[rd] 按键释放失败", error);
      return false;
    }
  }, [viewer.connected]);

  const closeViewer = useCallback(async () => {
    if (!guardRunning()) return;
    try {
      setViewer(await ipc.rdViewerClose());
    } catch (error) {
      toastError(String(error));
    }
  }, [guardRunning]);

  return {
    running,
    host,
    viewer,
    busy,
    lastError,
    startHost,
    stopHost,
    approve,
    deny,
    setQuality,
    connectViewer,
    closeViewer,
    onFrame,
    sendMouse,
    sendKey,
    resetKeys,
  };
}

export type UseRdPageModel = ReturnType<typeof useRdPageModel>;