// 分享链接导入状态机（§7 guest 导入，弹层/消息卡片共用）：
// 解析校验 → console /connect-share → 成功落连接目录（scope 徽章=分享 scope）。
import { useCallback, useState } from "react";

import { useAcpStore } from "./acp-store";
import { connectShare } from "./console-client";
import { parseShareLink, ShareLinkError, type ShareScope } from "./share-model";

export type ShareJoinPhase =
  | { state: "idle" }
  | { state: "invalid" }
  | { state: "needConsole" }
  | { state: "joining" }
  | { state: "joined"; peer: string; scope: ShareScope }
  | { state: "denied"; code: string | null; reason: string | null };

/** 显式本机连接面：调用方持有 console ready 快照时直传，免经 store draft 中转 */
export interface ShareJoinConnection {
  statusUrl: string;
  token: string;
}

function asScope(value: string | null): ShareScope {
  // Owner 不可经分享产生（§3）；响应缺 scope 回落 sandbox 徽章
  return value === "workspace" ? "workspace" : "sandbox";
}

export function useShareJoin() {
  const [phase, setPhase] = useState<ShareJoinPhase>({ state: "idle" });

  // 返回终态 phase：调用方可据 peer 落 saved endpoint（弹窗导入流）；
  // 旧调用方（加入卡/消息卡片）忽略返回值不受影响。
  const join = useCallback(async (
    rawLink: string,
    conn?: ShareJoinConnection,
  ): Promise<ShareJoinPhase> => {
    let parsed;
    try {
      parsed = parseShareLink(rawLink);
    } catch (error) {
      if (error instanceof ShareLinkError) {
        const next: ShareJoinPhase = { state: "invalid" };
        setPhase(next);
        return next;
      }
      throw error;
    }
    const link = rawLink.trim();
    const { draft, addManualPeer, setDirectoryScope } = useAcpStore.getState();
    const statusUrl = conn?.statusUrl ?? draft.statusUrl;
    const token = conn?.token ?? draft.token;
    if (!statusUrl?.trim() || !token.trim()) {
      const next: ShareJoinPhase = { state: "needConsole" };
      setPhase(next);
      return next;
    }
    setPhase({ state: "joining" });
    const outcome = await connectShare(statusUrl, token, link);
    if (!outcome.ok) {
      const next: ShareJoinPhase = { state: "denied", code: outcome.code, reason: outcome.reason };
      setPhase(next);
      return next;
    }
    const peer = outcome.peer ?? parsed.peer;
    const scope = asScope(outcome.scope);
    // 成功进连接目录（§8）：scope 徽章=分享 scope
    addManualPeer(peer);
    setDirectoryScope(peer, scope);
    const next: ShareJoinPhase = { state: "joined", peer, scope };
    setPhase(next);
    return next;
  }, []);

  const reset = useCallback(() => setPhase({ state: "idle" }), []);

  return { phase, join, reset };
}
