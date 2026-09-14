// ACS1 侧栏数据模型（纯函数，组件层只渲染）：端点清单与端点-会话归属。
// 数据面只用 acp-store 既有切片（saved / sessions / unread / lastInteraction），
// 不新增 IPC、不改协议：会话清单只属于「当前已连接端点」，其余端点显离线态。
import { LOCAL_AGENT_ENDPOINT_ID } from "@/acp/console-client";
import type { AcpEndpoint, AcpPhase, SessionSummary } from "@/acp/protocol";
import { wsHostOf } from "@/lib/conversation-entry";

export interface AgentEndpointRow {
  endpointId: string;
  label: string;
  wsUrl: string;
  local: boolean;
  unread: number;
  lastInteractionMs: number;
}

export interface EndpointRowsInput {
  saved: readonly AcpEndpoint[];
  localLabel: string;
  unreadByEndpoint: Record<string, number>;
  lastInteractionByEndpoint: Record<string, number>;
  /** console 快照存在（= 本机确有一个 agent 面）才补本机占位行，
   *  无快照时保持空清单，让「暂无端点」空态真实可达 */
  includeLocalPlaceholder: boolean;
}

/** 端点主键：endpointId 优先，草稿/存量条目回退 wsUrl（与 AgentConversation 同口径） */
export function endpointKey(endpoint: AcpEndpoint): string {
  return endpoint.endpointId?.trim() || endpoint.wsUrl;
}

/** 端点显示名：alias 优先，回退 wsUrl host，最后裸主键（P3#16 兜底） */
export function endpointLabel(endpoint: AcpEndpoint): string {
  return (
    endpoint.alias?.trim() ||
    wsHostOf(endpoint.wsUrl) ||
    endpoint.endpointId?.trim() ||
    endpoint.wsUrl
  );
}

/** 本机 agent 端点恒在清单内：console 尚未登记时补一行占位，保证 rail 入口与
 *  /agent?endpoint=acp-local-agent 深链不会落到空清单（未连接时由会话区显连接态）。 */
export function endpointRows(input: EndpointRowsInput): AgentEndpointRow[] {
  const rows = input.saved
    .map((endpoint) => ({
      endpointId: endpointKey(endpoint),
      label: endpointLabel(endpoint),
      wsUrl: endpoint.wsUrl,
      local: endpointKey(endpoint) === LOCAL_AGENT_ENDPOINT_ID,
      unread: input.unreadByEndpoint[endpointKey(endpoint)] ?? 0,
      lastInteractionMs: input.lastInteractionByEndpoint[endpointKey(endpoint)] ?? 0,
    }))
    .filter((row) => row.endpointId !== "");
  if (rows.some((row) => row.endpointId === LOCAL_AGENT_ENDPOINT_ID)) return rows;
  if (!input.includeLocalPlaceholder) return rows;
  return [
    {
      endpointId: LOCAL_AGENT_ENDPOINT_ID,
      label: input.localLabel,
      wsUrl: "",
      local: true,
      unread: input.unreadByEndpoint[LOCAL_AGENT_ENDPOINT_ID] ?? 0,
      lastInteractionMs: input.lastInteractionByEndpoint[LOCAL_AGENT_ENDPOINT_ID] ?? 0,
    },
    ...rows,
  ];
}

/** 端点会话清单：只归属已连接端点（store 只持有当前连接的会话表），
 *  未连接端点返回空表并由侧栏显离线提示，不伪造历史。 */
export function sessionsOfEndpoint(options: {
  sessions: readonly SessionSummary[];
  endpointId: string;
  activeEndpointId: string | null;
  phase: AcpPhase;
}): readonly SessionSummary[] {
  if (options.phase !== "online") return [];
  if (options.activeEndpointId !== options.endpointId) return [];
  return options.sessions;
}

/** 会话行相对时间：仅有「当前会话」的真实交互时刻可用（store 的
 *  lastInteraction 按端点记账，无 per-session 时刻）。其余行不显时间，
 *  禁止把端点的时刻冒充成每条会话的时刻。 */
export function sessionActivityMs(options: {
  sessionId: string;
  endpointId: string;
  activeEndpointId: string | null;
  activeSessionId: string | null;
  lastInteractionByEndpoint: Record<string, number>;
}): number {
  if (options.endpointId !== options.activeEndpointId) return 0;
  if (options.sessionId !== options.activeSessionId) return 0;
  return options.lastInteractionByEndpoint[options.endpointId] ?? 0;
}
