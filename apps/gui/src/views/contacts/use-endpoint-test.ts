import { useCallback, useEffect } from "react";

import { useAcpStore } from "@/acp/acp-store";
import { recordTestOutcome } from "@/acp/endpoint-meta";
import { newEndpointId } from "@/acp/endpoint-storage";
import type { AcpEndpoint } from "@/acp/protocol";
import { toastError } from "@/components/feedback/toast";
import i18n from "@/i18n";

/** 测试结论等待上限：拨号悬挂（对端不可达且 WS 无回报）时兜底转失败，
 *  并断开连接解除按钮的 loading 态，杜绝「连接中…」无限挂起 */
const TEST_TIMEOUT_MS = 12_000;

interface InFlightTest {
  endpointId: string;
  timer: ReturnType<typeof setTimeout>;
}

// 单连接语义 => 全局至多一个在途测试；模块态让 dialog/drawer 双挂载
// 共享同一份在途结论，双实例 effect 不会重复结算或重复 toast。
let inFlight: InFlightTest | null = null;

function dropInFlight(): void {
  if (!inFlight) return;
  clearTimeout(inFlight.timer);
  inFlight = null;
}

/** 在途测试结算（幂等）：结论落 meta + 失败 toast；closeInfo/lastError 取当下值 */
function settleTest(outcome: "ok" | "failed"): void {
  if (!inFlight) return;
  const { endpointId } = inFlight;
  dropInFlight();
  recordTestOutcome(endpointId, outcome);
  if (outcome !== "failed") return;
  const { closeInfo, lastError } = useAcpStore.getState();
  const detail =
    closeInfo && closeInfo.kind !== "closed"
      ? closeInfo.kind + " (code " + closeInfo.code + ")"
      : (lastError ?? undefined);
  toastError(i18n.t("contacts.endpoint.testFailed"), {
    description: detail,
    context: "acp.endpointTest",
  });
}

// 「测试连接」先行验证（§3.2）：拨号即测试，online = 通过；offline 终态、
// 命令拒绝（token/peer 缺失，idle+lastError）、进重连（初拨已败）与拨号
// 悬挂超时均判未通过。结论写入 endpoint-meta（最近连接结果口径，供行内
// 警告徽标）；未通过仍允许保存。呈现端订阅 meta 取行内反馈。
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
  // 在途测试优先结算；非测试期的相位迁移沿用原口径记徽标。
  useEffect(() => {
    if (!activeEndpointId) return;
    if (inFlight && inFlight.endpointId === activeEndpointId) {
      if (phase === "online") {
        settleTest("ok");
        return;
      }
      if (phase === "offline" || (phase === "idle" && lastError !== null)) {
        settleTest("failed");
        return;
      }
      // 测试首轮拨号 close 即进重连：说明初拨失败，重试必然同败，
      // 立即断开止损（避免重连风暴与长时间假 loading）
      if (phase === "reconnecting") {
        disconnect();
        settleTest("failed");
        return;
      }
      return;
    }
    if (phase === "online") {
      recordTestOutcome(activeEndpointId, "ok");
    } else if (phase === "offline") {
      recordTestOutcome(activeEndpointId, "failed");
    } else if (phase === "idle" && lastError !== null) {
      recordTestOutcome(activeEndpointId, "failed");
    }
  }, [activeEndpointId, phase, lastError, disconnect]);

  const start = useCallback(
    (endpoint: AcpEndpoint) => {
      const endpointId = endpoint.endpointId ?? newEndpointId();
      // 上一轮在途未决（如拨号悬挂）：先行清场再起新测
      dropInFlight();
      const timer = setTimeout(() => {
        // 兜底结算：拨号悬挂（connecting/reconnecting 未解）判失败并断开；
        // 已 online 的补记通过（观察组件可能已随路由卸载）；其余静默清场
        const { phase } = useAcpStore.getState();
        if (phase === "connecting" || phase === "reconnecting") {
          disconnect();
          settleTest("failed");
          return;
        }
        if (phase === "online") settleTest("ok");
        else dropInFlight();
      }, TEST_TIMEOUT_MS);
      inFlight = { endpointId, timer };
      setDraft({ ...endpoint, endpointId });
      // 单连接语义：丢弃现存连接后按该 endpoint 重新拨号
      disconnect();
      connect();
    },
    [setDraft, disconnect, connect],
  );

  return { start, testing: phase === "connecting" || phase === "reconnecting" };
}
