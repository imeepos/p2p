// store 事件接线：WS 事件到 store 状态的全部路由，以及连接实例的持有。
// 通知面：session/update（气泡/思考/工具时间线/配置/用量）+ dsh/bridge/reattach
//（续连补放横幅，apps/acp-agent/README.md 桥约定）。请求面：request_permission
// 进交互面等批准；其余 agent 请求按 JSON-RPC 礼节回 method-not-found，不静默吞。
import {
  AcpConnection,
  DEFAULT_RECONNECT,
  type AcpConnectionEvents,
} from "./acp-connection";
import {
  cancelledOutcome,
  REATTACH_METHOD,
  type AcpEndpoint,
  type AcpPhase,
  type AcpCloseInfo,
  type PermissionRequestParams,
  type ReattachParams,
  type SessionUpdateParams,
  type ConfigOption,
  type UsageUpdate,
} from "./protocol";
import {
  addPermission,
  applyConfigOptions,
  applyUsage,
  emptyInteraction,
  rejectUnanswered,
  type InteractionState,
} from "./interaction-model";
import { applyUpdate, emptyTranscript, type TranscriptState } from "./transcript-model";
import { resolveWsFactory } from "./ws-factory";
import { useAcpStore } from "./acp-store";
import i18n from "@/i18n";
import { toastError } from "@/components/feedback/toast";
import type { I18nKey } from "@/i18n/types";

let conn: AcpConnection | null = null;

/** lastError 码 -> 展示文案键：连接卡 Notices 与失败 toast 共用一份，避免双表漂移 */
export const ACP_ERROR_KEYS: Record<string, I18nKey> = {
  initializeFailed: "acp.errors.initializeFailed",
  endpointIncomplete: "acp.errors.endpointIncomplete",
  sessionNewFailed: "acp.errors.sessionNewFailed",
  sessionResumeFailed: "acp.errors.sessionResumeFailed",
  sessionCloseFailed: "acp.errors.sessionCloseFailed",
  promptFailed: "acp.errors.promptFailed",
  setConfigFailed: "acp.errors.setConfigFailed",
};

/** 动作失败的页面折射：sonner toast（与 chat 页一致）。
 *  console.warn 留痕由各调用点自行保留，这里只管上墙。 */
export function notifyActionFailure(code: string, error: unknown): void {
  const key = ACP_ERROR_KEYS[code];
  if (!key) return;
  toastError(i18n.t(key), {
    description: error instanceof Error ? error.message : String(error ?? ""),
    context: "acp." + code,
  });
}

export function currentConnection(): AcpConnection | null {
  return conn;
}

export function closeConnection(): void {
  conn?.close();
  conn = null;
}

export function mapTranscript(
  sessionId: string,
  fn: (t: TranscriptState) => TranscriptState,
): void {
  useAcpStore.setState((s) => ({
    transcripts: { ...s.transcripts, [sessionId]: fn(s.transcripts[sessionId] ?? emptyTranscript()) },
  }));
}

export function mapInteraction(
  sessionId: string,
  fn: (s: InteractionState) => InteractionState,
): void {
  useAcpStore.setState((state) => ({
    interactions: {
      ...state.interactions,
      [sessionId]: fn(state.interactions[sessionId] ?? emptyInteraction()),
    },
  }));
}

function resolveSessionId(explicit: unknown): string | null {
  if (typeof explicit === "string" && explicit !== "") return explicit;
  return useAcpStore.getState().activeSessionId;
}

function handleSessionUpdate(params: unknown): void {
  const p = params as SessionUpdateParams | null;
  if (!p || !p.update) return;
  const sessionId = resolveSessionId(p.sessionId);
  if (!sessionId) {
    console.warn("[acp] session/update 缺会话归属，已丢弃");
    return;
  }
  const update = p.update;
  // 联合末位留了 string 兜底变体，字面量窄化后仍混入兜底，取字段需显式收窄
  if (update.sessionUpdate === "config_option_update") {
    const options = (update as { configOptions?: ConfigOption[] }).configOptions;
    mapInteraction(sessionId, (s) => applyConfigOptions(s, options));
    return;
  }
  if (update.sessionUpdate === "usage_update") {
    mapInteraction(sessionId, (s) => applyUsage(s, update as UsageUpdate));
    return;
  }
  mapTranscript(sessionId, (t) => applyUpdate(t, update));
}

/** agent 侧请求：request_permission 登记进交互面；其余回 -32601 */
function handleAgentRequest(
  method: string,
  params: unknown,
  id: number,
  connection: AcpConnection | null,
): void {
  if (method === "session/request_permission") {
    const p = (params ?? {}) as PermissionRequestParams;
    const sessionId = resolveSessionId(p.sessionId);
    if (!sessionId) {
      console.warn("[acp] request_permission 无会话归属，按已拒绝处理");
      connection?.respond(id, cancelledOutcome());
      return;
    }
    mapInteraction(sessionId, (s) =>
      addPermission(s, {
        requestId: id,
        sessionId,
        title: p.toolCall?.title ?? p.toolCall?.toolCallId ?? String(id),
        toolKind: p.toolCall?.kind ?? null,
        options: Array.isArray(p.options) ? p.options : [],
        receivedAt: Date.now(),
      }),
    );
    return;
  }
  console.warn("[acp] 未支持的 agent 请求，回 method-not-found", method);
  connection?.respond(id, {
    error: { code: -32601, message: "method not found: " + method },
  });
}

function handleNotification(method: string, params: unknown): void {
  if (method === "session/update") {
    handleSessionUpdate(params);
    return;
  }
  if (method === REATTACH_METHOD) {
    const p = (params ?? {}) as ReattachParams;
    const replayed = typeof p.replayed === "number" ? p.replayed : 0;
    useAcpStore.setState({ reattachNotice: { replayed } });
    return;
  }
  // 其余通知（available_commands_update 等 v1 面）本卡无 UI 归宿，显式留痕
  console.debug("[acp] 未接通知", method);
}

async function runHandshake(connection: AcpConnection): Promise<void> {
  try {
    const capabilities = await connection.initialize();
    useAcpStore.setState({ capabilities });
  } catch (error) {
    console.warn("[acp] initialize 失败", error);
    notifyActionFailure("initializeFailed", error);
    useAcpStore.setState({ lastError: "initializeFailed" });
  }
  await useAcpStore.getState().refreshSessions();
}

/** 断流/掉线：未决权限显示为已拒绝（桥侧已 reject-once），不向已断的 socket 应答 */
function rejectPendingEverywhere(): void {
  useAcpStore.setState((s) => {
    const interactions = Object.fromEntries(
      Object.entries(s.interactions).map(([id, state]) => [id, rejectUnanswered(state)]),
    );
    return { interactions };
  });
}

export function wireAcpEvents(): AcpConnectionEvents {
  return {
    onPhase: (phase: AcpPhase) => {
      useAcpStore.setState((s) => ({
        phase,
        // 回在线或转 offline 终态后横幅即失效，清状态避免「第 x/y 次」常驻
        reconnect: phase === "online" || phase === "offline" ? null : s.reconnect,
      }));
      if (phase === "reconnecting" || phase === "offline") rejectPendingEverywhere();
    },
    onCloseInfo: (info: AcpCloseInfo) => useAcpStore.setState({ closeInfo: info }),
    onReconnect: (attempt: number, max: number) =>
      useAcpStore.setState({ reconnect: { attempt, max } }),
    onNotification: handleNotification,
    onRequest: (method, params, id) => handleAgentRequest(method, params, id, conn),
  };
}

/** 拨号并接线；每次进入 online（含自动重连）都重跑 initialize 重协商能力 */
export function startConnection(endpoint: AcpEndpoint): void {
  const base = wireAcpEvents();
  const events: AcpConnectionEvents = {
    ...base,
    onPhase: (phase) => {
      base.onPhase(phase);
      if (phase === "online" && conn) void runHandshake(conn);
    },
  };
  const connection = new AcpConnection(endpoint, resolveWsFactory(), events, DEFAULT_RECONNECT);
  conn = connection;
  connection.connect();
}