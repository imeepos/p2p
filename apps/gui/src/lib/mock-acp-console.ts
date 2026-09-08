// acp 泵 in-process 状态 mock（契约 §15）：与真实实现同签名（acp_console_status +
// acp-console 事件）。mock 的是 pump 内部状态机（connecting/connected/disconnected
// 三态），而非旧 sidecar 的外部进程生命周期。默认相位经 VITE_MOCK_CONSOLE_PHASE
// 钉死（缺省 connected 演示自动直达）；测试经 window.__MOCK_ACP_CONSOLE__ 控制器
// 驱动，非法/未知相位显式告警并回退 disconnected，不静默。
import type {
  AcpConsoleEventHandler,
  AcpConsolePhase,
  AcpConsoleStatus,
  UnlistenFn,
} from "./ipc-types";

const PHASES: AcpConsolePhase[] = ["connecting", "connected", "disconnected"];

function phaseFromEnv(): AcpConsolePhase {
  const raw = import.meta.env.VITE_MOCK_CONSOLE_PHASE as string | undefined;
  if (!raw) return "connected";
  if (!PHASES.includes(raw as AcpConsolePhase)) {
    console.warn("[mock-acp-console] 未知 VITE_MOCK_CONSOLE_PHASE:", raw);
    return "disconnected";
  }
  return raw as AcpConsolePhase;
}

// connected 相位给全会话连接面：wsUrl/token 与 mock WS（ws-factory）默认端点对齐，
// statusUrl 供发现面轮询（fetch 由测试/dev 注入桩）。
function initialStatus(): AcpConsoleStatus {
  const phase = phaseFromEnv();
  if (phase !== "connected") {
    return { phase };
  }
  return {
    phase: "connected",
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-console-token",
    statusUrl: "http://127.0.0.1:8788",
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
