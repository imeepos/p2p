import { useCallback, useEffect, useState } from "react";

import { useAcpStore } from "@/acp/acp-store";
import { recordTestOutcome, type EndpointTestOutcome } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";

// 「测试连接」先行验证（§3.2）：拨号即测试，online = 通过；offline 终态
// 或命令拒绝（token/peer 缺失等，phase 停留 idle 且 lastError 置位）=
// 未通过。结论写入 endpoint-meta 供行内警告徽标；允许未通过仍保存。
export interface EndpointTestResult {
  id: string;
  outcome: EndpointTestOutcome;
}

export function useEndpointTest(): {
  start: (endpointId: string, endpoint: AcpEndpoint) => void;
  testingId: string | null;
  result: EndpointTestResult | null;
} {
  const phase = useAcpStore((s) => s.phase);
  const lastError = useAcpStore((s) => s.lastError);
  const setDraft = useAcpStore((s) => s.setDraft);
  const connect = useAcpStore((s) => s.connect);
  const disconnect = useAcpStore((s) => s.disconnect);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [result, setResult] = useState<EndpointTestResult | null>(null);

  const settled: EndpointTestOutcome | null = testingId
    ? phase === "online"
      ? "ok"
      : phase === "offline" || (phase === "idle" && lastError !== null)
        ? "failed"
        : null
    : null;

  useEffect(() => {
    if (!testingId || !settled) return;
    recordTestOutcome(testingId, settled);
    setResult({ id: testingId, outcome: settled });
    setTestingId(null);
  }, [testingId, settled]);

  const start = useCallback(
    (endpointId: string, endpoint: AcpEndpoint) => {
      setDraft({ ...endpoint, endpointId });
      // 单连接语义：丢弃现存连接后按该 endpoint 重新拨号
      disconnect();
      connect();
      setResult(null);
      setTestingId(endpointId);
    },
    [setDraft, disconnect, connect],
  );

  return { start, testingId, result };
}
