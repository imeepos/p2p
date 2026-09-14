import { useCallback, useEffect, useRef, useState } from "react";

import { toastError } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";
import type { TunnelOpenReport, TunnelStatusReport } from "@/lib/ipc-types";

import type { TunnelPhase } from "./dsh-open-card";
import { isTunnelErrorCode } from "./tunnel-flow";
import type { TunnelBannerInput } from "./tunnel-terminal-banner";

const IDLE_STATUS: TunnelStatusReport = {
  active: false,
  localAddr: null,
  target: null,
  sessions: [],
  serve: { enabled: false, allow: [], activeSessions: 0 },
};

// 远程访问页状态模型：快照/事件同步 + 入口终态汇聚 + 运行中失败横幅。
// 运行中失败消费 tunnel_status 事件 outcome∈六值闭集的终态记录（S7），
// 按 sessionId 去重，同一终态记录不重复告警。
export function useTunnelPageModel() {
  const [status, setStatus] = useState<TunnelStatusReport>(IDLE_STATUS);
  const [openUrl, setOpenUrl] = useState<string | null>(null);
  const [phase, setPhase] = useState<TunnelPhase>("idle");
  const [banner, setBanner] = useState<TunnelBannerInput | null>(null);
  const banneredSessionRef = useRef<string | null>(null);

  // 事件终态 outcome∈六值闭集（非 open/ok）→ 失败横幅（人话+code 由横幅映射）。
  const raiseRuntimeFailure = useCallback((next: TunnelStatusReport) => {
    const last = next.sessions[next.sessions.length - 1];
    if (!last || !isTunnelErrorCode(last.outcome)) return;
    if (banneredSessionRef.current === last.sessionId) return;
    banneredSessionRef.current = last.sessionId;
    setBanner({ tone: "error", code: last.outcome });
  }, []);

  useEffect(() => {
    let alive = true;
    ipc
      .tunnelStatus()
      .then((current) => {
        if (!alive) return;
        setStatus(current);
        // 错误态不回退：快照仅在没有更新的错误展示时生效。
        setPhase((prev) =>
          prev === "error" ? prev : current.active ? "open" : "idle",
        );
      })
      .catch((error) => console.error("[tunnel] 状态读取失败", error));
    const unlisten = ipc.onTunnelStatus((next) => {
      setStatus(next);
      setPhase(next.active ? "open" : "idle");
      raiseRuntimeFailure(next);
    });
    return () => {
      alive = false;
      void unlisten.then((off) => off());
    };
    // 挂载时订阅一次；开/关状态由命令返回值与事件驱动。
  }, [raiseRuntimeFailure]);

  // 入口成功回调（DSH 与通用共用）：汇入页面共享状态。
  const handleOpened = useCallback((report: TunnelOpenReport) => {
    setOpenUrl(report.openUrl);
    setStatus((prev) => ({ ...prev, active: true, localAddr: report.localAddr }));
    setPhase("open");
  }, []);

  const handleError = useCallback((message: string) => {
    setPhase("error");
    toastError(message);
  }, []);

  const handleServeUpdate = useCallback(
    (serve: TunnelStatusReport["serve"]) => {
      setStatus((prev) => ({ ...prev, serve }));
    },
    [],
  );

  const dismissBanner = useCallback(() => setBanner(null), []);

  return {
    status,
    phase,
    openUrl,
    banner,
    setBanner,
    handleOpened,
    handleError,
    handleServeUpdate,
    dismissBanner,
  };
}
