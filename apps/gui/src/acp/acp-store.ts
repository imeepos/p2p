// ACP 控制台 store：连接生命周期 + 会话侧栏 + transcript。
// 会话清单与 transcript 常驻内存，跨断线重连存活（设计 §8 侧栏行）；
// 仅 resetConsoleState 或显式清会话时移除。
import { create } from "zustand";

import {
  AcpConnection,
  DEFAULT_RECONNECT,
  type AcpConnectionEvents,
} from "./acp-connection";
import {
  type AcpCloseInfo,
  type AcpEndpoint,
  type AcpPhase,
  type InitializeResult,
  type SessionSummary,
  type SessionUpdateParams,
} from "./protocol";
import {
  applyUpdate,
  applyUserPrompt,
  emptyTranscript,
  settleTranscript,
  toggleThought,
  type TranscriptState,
} from "./transcript-model";
import { loadStored, persistStored } from "./endpoint-storage";
import { resolveWsFactory } from "./ws-factory";

let conn: AcpConnection | null = null;

interface AcpConsoleState {
  phase: AcpPhase;
  draft: AcpEndpoint;
  saved: AcpEndpoint[];
  activePeer: string | null;
  closeInfo: AcpCloseInfo | null;
  reconnect: { attempt: number; max: number } | null;
  capabilities: InitializeResult | null;
  sessions: SessionSummary[];
  activeSessionId: string | null;
  transcripts: Record<string, TranscriptState>;
  promptPending: boolean;
  lastError: string | null;
  setDraft: (patch: Partial<AcpEndpoint>) => void;
  saveDraft: () => void;
  removeSaved: (peer: string) => void;
  connect: () => Promise<void>;
  disconnect: () => void;
  newSession: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  resumeSession: (sessionId: string) => Promise<void>;
  closeSession: (sessionId: string) => Promise<void>;
  sendPrompt: (text: string) => Promise<void>;
  cancelPrompt: () => void;
  toggleThought: (sessionId: string, turnId: number) => void;
  resetConsoleState: () => void;
}

function mapTranscript(
  sessionId: string,
  fn: (t: TranscriptState) => TranscriptState,
): void {
  useAcpStore.setState((s) => ({
    transcripts: { ...s.transcripts, [sessionId]: fn(s.transcripts[sessionId] ?? emptyTranscript()) },
  }));
}

function handleUpdate(params: unknown): void {
  const p = params as SessionUpdateParams | null;
  if (!p || !p.update) return;
  const sessionId =
    typeof p.sessionId === "string" ? p.sessionId : useAcpStore.getState().activeSessionId;
  if (!sessionId) {
    console.warn("[acp] session/update 缺会话归属，已丢弃");
    return;
  }
  const update = p.update;
  mapTranscript(sessionId, (t) => applyUpdate(t, update));
}

async function runHandshake(connection: AcpConnection): Promise<void> {
  try {
    const capabilities = await connection.initialize();
    useAcpStore.setState({ capabilities });
  } catch (error) {
    console.warn("[acp] initialize 失败", error);
    useAcpStore.setState({ lastError: "initializeFailed" });
  }
  await useAcpStore.getState().refreshSessions();
}

function wireEvents(): AcpConnectionEvents {
  return {
    onPhase: (phase) => useAcpStore.setState({ phase }),
    onCloseInfo: (info) => useAcpStore.setState({ closeInfo: info }),
    onReconnect: (attempt, max) => useAcpStore.setState({ reconnect: { attempt, max } }),
    onNotification: (method, params) => {
      if (method === "session/update") handleUpdate(params);
    },
  };
}

const stored = loadStored();

export const useAcpStore = create<AcpConsoleState>()((set, get) => ({
  phase: "idle",
  draft: stored.draft,
  saved: stored.saved,
  activePeer: null,
  closeInfo: null,
  reconnect: null,
  capabilities: null,
  sessions: [],
  activeSessionId: null,
  transcripts: {},
  promptPending: false,
  lastError: null,

  setDraft: (patch) => {
    const draft = { ...get().draft, ...patch };
    set({ draft });
    persistStored({ draft, saved: get().saved });
  },

  saveDraft: () => {
    const { draft, saved } = get();
    if (!draft.peer || saved.some((e) => e.peer === draft.peer && e.wsUrl === draft.wsUrl)) return;
    const next = [...saved, draft];
    set({ saved: next });
    persistStored({ draft, saved: next });
  },

  removeSaved: (peer) => {
    const next = get().saved.filter((e) => e.peer !== peer);
    set({ saved: next });
    persistStored({ draft: get().draft, saved: next });
  },

  connect: async () => {
    // 上次连接失败（idle/offline）后允许重连：丢弃旧连接对象重新拨号
    const currentPhase = get().phase;
    if (conn && (currentPhase === "idle" || currentPhase === "offline")) {
      conn.close();
      conn = null;
    }
    if (conn) return;
    const draft = get().draft;
    if (!draft.token || !draft.peer) {
      set({ lastError: "endpointIncomplete" });
      return;
    }
    set({
      phase: "connecting",
      closeInfo: null,
      reconnect: null,
      capabilities: null,
      lastError: null,
      activePeer: draft.peer,
    });
    const events = wireEvents();
    const connection = new AcpConnection(draft, resolveWsFactory(), {
      ...events,
      onPhase: (phase) => {
        events.onPhase(phase);
        // 每次进入 online（含自动重连）都重跑 initialize：agent 侧连接重建后能力需重新协商
        if (phase === "online") void runHandshake(connection);
      },
    }, DEFAULT_RECONNECT);
    conn = connection;
    connection.connect();
  },

  disconnect: () => {
    conn?.close();
    conn = null;
    set({ phase: "idle", promptPending: false, reconnect: null });
  },

  newSession: async () => {
    if (!conn) return;
    try {
      const r = await conn.sessionNew();
      set({ activeSessionId: r.sessionId });
      await get().refreshSessions();
    } catch (error) {
      console.warn("[acp] session/new 失败", error);
      set({ lastError: "sessionNewFailed" });
    }
  },

  refreshSessions: async () => {
    if (!conn) return;
    try {
      const r = await conn.sessionList();
      set({ sessions: r.sessions ?? [] });
    } catch (error) {
      console.warn("[acp] session/list 失败，保留现有清单", error);
    }
  },

  resumeSession: async (sessionId) => {
    if (!conn) return;
    try {
      await conn.sessionResume(sessionId);
      set({ activeSessionId: sessionId });
    } catch (error) {
      console.warn("[acp] session/resume 失败", error);
      set({ lastError: "sessionResumeFailed" });
    }
  },

  closeSession: async (sessionId) => {
    if (!conn) return;
    try {
      await conn.sessionDelete(sessionId);
    } catch (error) {
      console.warn("[acp] session/delete 失败", error);
      set({ lastError: "sessionCloseFailed" });
    }
    set((s) => ({
      sessions: s.sessions.filter((e) => e.sessionId !== sessionId),
      activeSessionId: s.activeSessionId === sessionId ? null : s.activeSessionId,
      transcripts: Object.fromEntries(
        Object.entries(s.transcripts).filter(([id]) => id !== sessionId),
      ),
    }));
  },

  sendPrompt: async (text) => {
    const { activeSessionId, promptPending } = get();
    const connection = conn;
    if (!activeSessionId || promptPending || !connection) return;
    const trimmed = text.trim();
    if (!trimmed) return;
    const sessionId = activeSessionId;
    mapTranscript(sessionId, (t) => applyUserPrompt(t, trimmed));
    set({ promptPending: true });
    try {
      const result = await connection.sessionPrompt(sessionId, trimmed);
      set({ promptPending: false });
      mapTranscript(sessionId, (t) => settleTranscript(t, result.stopReason ?? "end_turn"));
    } catch (error) {
      console.warn("[acp] prompt 失败", error);
      set({ promptPending: false, lastError: "promptFailed" });
      mapTranscript(sessionId, (t) => settleTranscript(t, null));
    }
  },

  cancelPrompt: () => {
    const sessionId = get().activeSessionId;
    if (sessionId) conn?.sessionCancel(sessionId);
  },

  toggleThought: (sessionId, turnId) => {
    mapTranscript(sessionId, (t) => toggleThought(t, turnId));
  },

  resetConsoleState: () => {
    conn?.close();
    conn = null;
    set({
      phase: "idle",
      activePeer: null,
      closeInfo: null,
      reconnect: null,
      capabilities: null,
      sessions: [],
      activeSessionId: null,
      transcripts: {},
      promptPending: false,
      lastError: null,
    });
  },
}));
