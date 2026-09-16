import { useCallback, useEffect, useState } from "react";

import { toastError } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";
import type { RdHostStatus, RdViewerStatus } from "@/lib/ipc-types";
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
  const running = useNodeStore((s) => s.status?.running === true);
  const [hostRaw, setHost] = useState<RdHostStatus>(IDLE_HOST);
  const [viewerRaw, setViewer] = useState<RdViewerStatus>(IDLE_VIEWER);
  const [busy, setBusy] = useState<"start" | "stop" | "connect" | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

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
      const message = "请先在网络页启动本机 p2p 节点";
      setLastError(message);
      return false;
    }
    return true;
  }, [running]);

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
        setViewer(await ipc.rdViewerConnect(peer));
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
  };
}

export type UseRdPageModel = ReturnType<typeof useRdPageModel>;