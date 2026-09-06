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

function asScope(value: string | null): ShareScope {
  // Owner 不可经分享产生（§3）；响应缺 scope 回落 sandbox 徽章
  return value === "workspace" ? "workspace" : "sandbox";
}

export function useShareJoin() {
  const [phase, setPhase] = useState<ShareJoinPhase>({ state: "idle" });

  const join = useCallback(async (rawLink: string) => {
    let parsed;
    try {
      parsed = parseShareLink(rawLink);
    } catch (error) {
      if (error instanceof ShareLinkError) {
        setPhase({ state: "invalid" });
        return;
      }
      throw error;
    }
    const link = rawLink.trim();
    const { draft, addManualPeer, setDirectoryScope } = useAcpStore.getState();
    if (!draft.statusUrl?.trim() || !draft.token.trim()) {
      setPhase({ state: "needConsole" });
      return;
    }
    setPhase({ state: "joining" });
    const outcome = await connectShare(draft.statusUrl, draft.token, link);
    if (!outcome.ok) {
      setPhase({ state: "denied", code: outcome.code, reason: outcome.reason });
      return;
    }
    const peer = outcome.peer ?? parsed.peer;
    const scope = asScope(outcome.scope);
    // 成功进连接目录（§8）：scope 徽章=分享 scope
    addManualPeer(peer);
    setDirectoryScope(peer, scope);
    setPhase({ state: "joined", peer, scope });
  }, []);

  const reset = useCallback(() => setPhase({ state: "idle" }), []);

  return { phase, join, reset };
}
