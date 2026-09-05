// 发现清单轮询（设计 §8 连接目录）：console /discovery 定期拉取进目录模型。
// 页面可见且目录挂载时轮询；退后台暂停、console 不可达停止（空态引导，不误报错误）。
import { useEffect } from "react";

import { fetchDiscoveryPeers } from "./console-client";
import { useAcpStore } from "./acp-store";

export const DISCOVERY_POLL_INTERVAL_MS = 5_000;

export function useDiscoveryPoll(params: {
  statusUrl: string | null;
  token: string;
}): void {
  const { statusUrl, token } = params;
  useEffect(() => {
    if (!statusUrl || !token) return;
    let timer: ReturnType<typeof setInterval> | null = null;
    let dead = false;

    const stop = () => {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
    };
    const poll = async () => {
      const peers = await fetchDiscoveryPeers(statusUrl, token);
      if (dead) return;
      if (peers === null) {
        // console 不可达：停止轮询，目录走既有空态引导，不弹错误
        stop();
        return;
      }
      useAcpStore.getState().ingestDiscovery(peers);
    };
    const startPolling = () => {
      if (timer || dead || document.hidden) return;
      void poll();
      timer = setInterval(() => void poll(), DISCOVERY_POLL_INTERVAL_MS);
    };
    const onVisibility = () => {
      if (document.hidden) stop();
      else startPolling();
    };
    document.addEventListener("visibilitychange", onVisibility);
    startPolling();
    return () => {
      dead = true;
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [statusUrl, token]);
}
