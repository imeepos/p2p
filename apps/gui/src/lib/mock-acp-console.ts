// acp-console 托管 mock（契约 v10 §15）：与真实实现同签名（acp_console_status +
// acp-console 事件）。相位可测可控——默认相位经 VITE_MOCK_CONSOLE_PHASE 钉死
//（缺省 ready 演示自动直达）；测试经 window.__MOCK_ACP_CONSOLE__ 控制器驱动，
// 非法/未知相位显式告警并回退 unavailable，不静默。
import type {
  AcpConsoleEventHandler,
  AcpConsolePhase,
  AcpConsoleStatus,
  UnlistenFn,
} from "./ipc-types";

const PHASES: AcpConsolePhase[] = [
  "starting",
  "ready",
  "restarting",
  "failed",
  "unavailable",
  "stopped",
];

function phaseFromEnv(): AcpConsolePhase {
  const raw = import.meta.env.VITE_MOCK_CONSOLE_PHASE as string | undefined;
  if (!raw) return "ready";
  if (!PHASES.includes(raw as AcpConsolePhase)) {
    console.warn("[mock-acp-console] 未知 VITE_MOCK_CONSOLE_PHASE:", raw);
    return "unavailable";
  }
  return raw as AcpConsolePhase;
}

// ready 相位给全会话连接面：wsUrl/token 与 mock WS（ws-factory）默认端点对齐，
// statusUrl 供发现面轮询（fetch 由测试/dev 注入桩）。
function initialStatus(): AcpConsoleStatus {
  const phase = phaseFromEnv();
  if (phase !== "ready") {
    return { phase, restarts: phase === "failed" ? 5 : 0 };
  }
  return {
    phase: "ready",
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-console-token",
    statusUrl: "http://127.0.0.1:8788",
    adminUrl: "http://127.0.0.1:8790",
    restarts: 0,
  };
}

let current: AcpConsoleStatus = initialStatus();
const handlers = new Set<AcpConsoleEventHandler>();

export const mockAcpConsole = {
  status(): AcpConsoleStatus {
    return { ...current };
  },
  /** 相位驱动入口：更新快照并广播（对齐真实事件语义：phase 变更即发射） */
  emit(status: AcpConsoleStatus): void {
    current = { ...status };
    for (const handler of handlers) handler(current);
  },
  subscribe(handler: AcpConsoleEventHandler): UnlistenFn {
    handlers.add(handler);
    return () => {
      handlers.delete(handler);
    };
  },
  reset(): void {
    current = initialStatus();
    handlers.clear();
  },
};

// 测试/dev 注入入口：控制台或测试脚本经 window 驱动相位矩阵。
(window as unknown as Record<string, unknown>).__MOCK_ACP_CONSOLE__ = mockAcpConsole;
