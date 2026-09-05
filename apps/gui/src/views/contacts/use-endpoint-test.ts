import { useCallback, useEffect } from "react";

import { useAcpStore } from "@/acp/acp-store";
import { recordTestOutcome } from "@/acp/endpoint-meta";
import { newEndpointId } from "@/acp/endpoint-storage";
import type { AcpEndpoint } from "@/acp/protocol";

// 「测试连接」先行验证（§3.2）：拨号即测试，online = 通过；offline 终态
// 或命令拒绝（token/peer 缺失等，phase 停留 idle 且 lastError 置位）=
// 未通过。结论写入 endpoint-meta（最近连接结果口径，供行内警告徽标）；
// 未通过仍允许保存。本钩子不自持可变状态：结果即时落 meta，呈现端自
// 行订阅（避免 effect 内 setState，react-hooks 纪律）。
export function useEndpointTest(): {
  start: (endpoint: AcpEndpoint) => void;
  testing: boolean;
} {
  const phase = useAcpStore((s) => s.phase);
  const lastError = useAcpStore((s) => s.lastError);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const setDraft = useAcpStore((s) => s.setDraft);
  const connect = useAcpStore((s) => s.connect);
  const disconnect = useAcpStore((s) => s.disconnect);

  // 连接结果落档（外部系统写入）：仅对该端点为活动连接时的终态生效。
  // 缺失 token/peer 的命令拒绝（idle+lastError）同样记为未通过。
  useEffect(() => {
    if (!activeEndpointId) return;
    if (phase === "online") {
      recordTestOutcome(activeEndpointId, "ok");
    } else if (phase === "offline") {
      recordTestOutcome(activeEndpointId, "failed");
    } else if (phase === "idle" && lastError !== null) {
      recordTestOutcome(activeEndpointId, "failed");
    }
  }, [activeEndpointId, phase, lastError]);

  const start = useCallback(
    (endpoint: AcpEndpoint) => {
      setDraft({ ...endpoint, endpointId: endpoint.endpointId ?? newEndpointId() });
      // 单连接语义：丢弃现存连接后按该 endpoint 重新拨号
      disconnect();
      connect();
    },
    [setDraft, disconnect, connect],
  );

  return { start, testing: phase === "connecting" || phase === "reconnecting" };
}
