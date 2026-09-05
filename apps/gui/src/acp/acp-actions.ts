// store 动作实现：凡需触碰连接实例（conn）或做跨切片归并的动作都在此，
// acp-store 只留状态与一行委托，避免单文件超限。失败路径一律 console.warn 留痕。
import { cancelledOutcome, selectedOutcome } from "./protocol";
import {
  applyConfigOptions,
  rejectUnanswered,
  resolvePermission,
} from "./interaction-model";
import { applyUserPrompt, settleTranscript } from "./transcript-model";
import {
  closeConnection,
  currentConnection,
  mapInteraction,
  mapTranscript,
  notifyActionFailure,
  startConnection,
} from "./store-events";
import { useAcpStore } from "./acp-store";

function get() {
  return useAcpStore.getState();
}

/** 未决权限收尾：respond=true 时按 ACP 约定回 cancelled（轮次取消场景） */
export function rejectPendingPermissions(sessionId: string, respond: boolean): void {
  const list = get().interactions[sessionId]?.permissions ?? [];
  const conn = currentConnection();
  for (const req of list) {
    if (req.status === "pending" && respond && conn) {
      conn.respond(req.requestId, cancelledOutcome());
    }
  }
  mapInteraction(sessionId, (s) => rejectUnanswered(s));
}

export function startConnect(): void {
  const existing = currentConnection();
  if (existing) {
    const phase = get().phase;
    // 上次连接失败（idle/offline）后允许重连：丢弃旧连接对象重新拨号
    if (phase !== "idle" && phase !== "offline") return;
    closeConnection();
  }
  const draft = get().draft;
  if (!draft.token || !draft.peer) {
    useAcpStore.setState({ lastError: "endpointIncomplete" });
    return;
  }
  useAcpStore.setState({
    phase: "connecting",
    closeInfo: null,
    reconnect: null,
    reattachNotice: null,
    sessionLostNotice: false,
    capabilities: null,
    lastError: null,
    activePeer: draft.peer,
  });
  startConnection(draft);
}

/** 手动立即重试入口：不打断自动重连计数，仅提前拨号 */
export function runRetryNow(): void {
  currentConnection()?.retryNow();
}

export function runDisconnect(): void {
  const sessionId = get().activeSessionId;
  if (sessionId) rejectPendingPermissions(sessionId, true);
  closeConnection();
  useAcpStore.setState({
    phase: "idle",
    promptPendingBySession: {},
    reconnect: null,
    reattachNotice: null,
    sessionLostNotice: false,
  });
}

export async function runNewSession(): Promise<void> {
  const conn = currentConnection();
  if (!conn) return;
  try {
    const r = (await conn.sessionNew()) as { sessionId: string; configOptions?: import("./protocol").ConfigOption[] };
    useAcpStore.setState({ activeSessionId: r.sessionId });
    mapInteraction(r.sessionId, (s) => applyConfigOptions(s, r.configOptions));
    await get().refreshSessions();
  } catch (error) {
    console.warn("[acp] session/new 失败", error);
    notifyActionFailure("sessionNewFailed", error);
    useAcpStore.setState({ lastError: "sessionNewFailed" });
  }
}

export async function runRefreshSessions(): Promise<void> {
  const conn = currentConnection();
  if (!conn) return;
  try {
    const r = (await conn.sessionList()) as { sessions?: { sessionId: string; title?: string; cwd?: string }[] };
    useAcpStore.setState({ sessions: r.sessions ?? [] });
  } catch (error) {
    console.warn("[acp] session/list 失败，保留现有清单", error);
  }
}

export async function runResumeSession(sessionId: string): Promise<void> {
  const conn = currentConnection();
  if (!conn) return;
  try {
    await conn.sessionResume(sessionId);
    useAcpStore.setState({ activeSessionId: sessionId });
  } catch (error) {
    console.warn("[acp] session/resume 失败", error);
    notifyActionFailure("sessionResumeFailed", error);
    useAcpStore.setState({ lastError: "sessionResumeFailed" });
  }
}

export async function runCloseSession(sessionId: string): Promise<void> {
  const conn = currentConnection();
  if (!conn) return;
  try {
    await conn.sessionDelete(sessionId);
  } catch (error) {
    console.warn("[acp] session/delete 失败", error);
    notifyActionFailure("sessionCloseFailed", error);
    useAcpStore.setState({ lastError: "sessionCloseFailed" });
    // agent 侧会话仍在：失败不移除本地条目，防侧栏复活幽灵
    return;
  }
  useAcpStore.setState((s) => {
    const sessions = s.sessions.filter((e) => e.sessionId !== sessionId);
    // 焦点回退：关掉的是当前会话时自动落到列表下一个，无会话才回空态
    const nextActive =
      s.activeSessionId === sessionId ? (sessions[0]?.sessionId ?? null) : s.activeSessionId;
    return {
      sessions,
      activeSessionId: nextActive,
      transcripts: Object.fromEntries(
        Object.entries(s.transcripts).filter(([id]) => id !== sessionId),
      ),
      interactions: dropSession(s.interactions, sessionId),
    };
  });
}

function dropSession(
  interactions: Record<string, import("./interaction-model").InteractionState>,
  sessionId: string,
): Record<string, import("./interaction-model").InteractionState> {
  if (!(sessionId in interactions)) return interactions;
  const next = { ...interactions };
  delete next[sessionId];
  return next;
}

export async function runSendPrompt(text: string): Promise<boolean> {
  const { activeSessionId } = get();
  const conn = currentConnection();
  if (!activeSessionId || !conn) return false;
  if (get().promptPendingBySession[activeSessionId]) return false;
  const trimmed = text.trim();
  if (!trimmed) return false;
  const sessionId = activeSessionId;
  const settlePending = (pending: boolean) =>
    useAcpStore.setState((s) => ({
      promptPendingBySession: { ...s.promptPendingBySession, [sessionId]: pending },
    }));
  mapTranscript(sessionId, (t) => applyUserPrompt(t, trimmed));
  settlePending(true);
  try {
    const result = (await conn.sessionPrompt(sessionId, trimmed)) as { stopReason?: string };
    settlePending(false);
    mapTranscript(sessionId, (t) => settleTranscript(t, result.stopReason ?? "end_turn"));
    return true;
  } catch (error) {
    console.warn("[acp] prompt 失败", error);
    notifyActionFailure("promptFailed", error);
    settlePending(false);
    useAcpStore.setState({ lastError: "promptFailed" });
    // 失败结算写约定值 "error"：气泡承载失败徽章（渲染归 P2），不再与正常结束同形
    mapTranscript(sessionId, (t) => settleTranscript(t, "error"));
    return false;
  }
}

export function runCancelPrompt(): void {
  const sessionId = get().activeSessionId;
  if (!sessionId) return;
  // 该会话没有进行中的回合不发取消，避免切会话后误杀
  if (!get().promptPendingBySession[sessionId]) return;
  currentConnection()?.sessionCancel(sessionId);
  // ACP 约定：轮次取消时未决 request_permission 必须回 cancelled
  rejectPendingPermissions(sessionId, true);
}

/** 选项制应答：点哪档回哪档 selected outcome；optionId=null（通用拒绝）回 cancelled */
export function runRespondPermission(requestId: number, optionId: string | null): void {
  const { activeSessionId, interactions } = get();
  if (!activeSessionId) return;
  const req = (interactions[activeSessionId]?.permissions ?? []).find(
    (p) => p.requestId === requestId,
  );
  if (!req || req.status !== "pending") return;
  const conn = currentConnection();
  if (conn) {
    const outcome = optionId ? selectedOutcome(optionId) : cancelledOutcome();
    conn.respond(requestId, outcome);
  }
  const opt = req.options.find((o) => o.optionId === optionId);
  const status = optionId && opt?.kind.startsWith("allow") ? "approved" : "rejected";
  mapInteraction(activeSessionId, (s) => resolvePermission(s, requestId, status));
}

export async function runSetConfigOption(configId: string, value: string | boolean): Promise<void> {
  const { activeSessionId } = get();
  const conn = currentConnection();
  if (!activeSessionId || !conn) return;
  try {
    const r = await conn.setConfigOption(activeSessionId, configId, value);
    mapInteraction(activeSessionId, (s) => applyConfigOptions(s, r.configOptions));
  } catch (error) {
    console.warn("[acp] set_config_option 失败", error);
    notifyActionFailure("setConfigFailed", error);
    useAcpStore.setState({ lastError: "setConfigFailed" });
  }
}