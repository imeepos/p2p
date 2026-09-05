// ACP 控制台 store：状态与一行委托。连接实例持有、事件路由在 store-events，
// 动作实现在 acp-actions；本文件只拼装（设计 §8：会话侧栏与 transcript 跨重连存活）。
import { create } from "zustand";

import type { AcpCloseInfo, AcpEndpoint, AcpPhase, InitializeResult, SessionSummary } from "./protocol";
import type { InteractionState } from "./interaction-model";
import type { AcpScope, DirectoryEntry, DiscoveryPeer } from "./directory-model";
import {
  addManual,
  removeEntry,
  setEntryScope,
  upsertDiscovered,
} from "./directory-model";
import { emptyTranscript, toggleThought, type TranscriptState } from "./transcript-model";
import { loadStored, persistStored } from "./endpoint-storage";
import { closeConnection } from "./store-events";
import {
  runCancelPrompt,
  runCloseSession,
  runDisconnect,
  runNewSession,
  runRefreshSessions,
  runRespondPermission,
  runResumeSession,
  runRetryNow,
  runSendPrompt,
  runSetConfigOption,
  startConnect,
} from "./acp-actions";

interface AcpConsoleState {
  phase: AcpPhase;
  draft: AcpEndpoint;
  saved: AcpEndpoint[];
  activePeer: string | null;
  closeInfo: AcpCloseInfo | null;
  reconnect: { attempt: number; max: number } | null;
  /** dsh/bridge/reattach 通知折射的续连补放横幅 */
  reattachNotice: { replayed: number } | null;
  /** 续连窗口过期（fresh 重连）后的原会话失效引导 */
  sessionLostNotice: boolean;
  capabilities: InitializeResult | null;
  sessions: SessionSummary[];
  activeSessionId: string | null;
  transcripts: Record<string, TranscriptState>;
  interactions: Record<string, InteractionState>;
  directory: DirectoryEntry[];
  /** prompt 回合进行中，按会话归属记录（切会话不误显 Stop/误发取消） */
  promptPendingBySession: Record<string, boolean>;
  /** 每会话未发送草稿：切会话互不污染，杜绝把话发给另一个 agent（P1 草稿隔离） */
  promptDrafts: Record<string, string>;
  lastError: string | null;
  setPromptDraft: (sessionId: string, text: string) => void;
  setDraft: (patch: Partial<AcpEndpoint>) => void;
  saveDraft: () => void;
  removeSaved: (peer: string) => void;
  connect: () => void;
  disconnect: () => void;
  /** 断线重连横幅的立即重试入口（不打断自动重连计数） */
  retryNow: () => void;
  /** 续连补放横幅手动关闭 */
  dismissReattachNotice: () => void;
  /** 原会话失效引导手动关闭 */
  dismissSessionLostNotice: () => void;
  newSession: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  resumeSession: (sessionId: string) => Promise<void>;
  closeSession: (sessionId: string) => Promise<void>;
  sendPrompt: (text: string) => Promise<boolean>;
  cancelPrompt: () => void;
  toggleThought: (sessionId: string, turnId: number) => void;
  respondPermission: (requestId: number, optionId: string | null) => void;
  setConfigOption: (configId: string, value: string | boolean) => Promise<void>;
  ingestDiscovery: (peers: DiscoveryPeer[]) => void;
  addManualPeer: (peer: string) => void;
  removeDirectoryEntry: (peer: string) => void;
  setDirectoryScope: (peer: string, scope: AcpScope) => void;
  resetConsoleState: () => void;
}

const stored = loadStored();

export const useAcpStore = create<AcpConsoleState>()((set, get) => ({
  phase: "idle",
  draft: stored.draft,
  saved: stored.saved,
  activePeer: null,
  closeInfo: null,
  reconnect: null,
  reattachNotice: null,
  sessionLostNotice: false,
  capabilities: null,
  sessions: [],
  activeSessionId: null,
  transcripts: {},
  interactions: {},
  directory: [],
  promptPendingBySession: {},
  promptDrafts: {},
  lastError: null,

  setPromptDraft: (sessionId, text) => {
    set((s) => ({ promptDrafts: { ...s.promptDrafts, [sessionId]: text } }));
  },

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

  connect: () => startConnect(),
  disconnect: () => runDisconnect(),
  retryNow: () => runRetryNow(),
  dismissReattachNotice: () => set({ reattachNotice: null }),
  dismissSessionLostNotice: () => set({ sessionLostNotice: false }),
  newSession: () => runNewSession(),
  refreshSessions: () => runRefreshSessions(),
  resumeSession: (sessionId) => runResumeSession(sessionId),
  closeSession: (sessionId) => runCloseSession(sessionId),
  sendPrompt: (text) => runSendPrompt(text),
  cancelPrompt: () => runCancelPrompt(),
  toggleThought: (sessionId, turnId) =>
    useAcpStore.setState((s) => ({
      transcripts: {
        ...s.transcripts,
        [sessionId]: toggleThought(s.transcripts[sessionId] ?? emptyTranscript(), turnId),
      },
    })),
  respondPermission: (requestId, optionId) => runRespondPermission(requestId, optionId),
  setConfigOption: (configId, value) => runSetConfigOption(configId, value),
  ingestDiscovery: (peers) =>
    set((s) => ({ directory: upsertDiscovered(s.directory, peers) })),
  addManualPeer: (peer) => {
    const trimmed = peer.trim();
    if (!trimmed) return;
    set((s) => ({ directory: addManual(s.directory, trimmed) }));
  },
  removeDirectoryEntry: (peer) =>
    set((s) => ({ directory: removeEntry(s.directory, peer) })),
  setDirectoryScope: (peer, scope) =>
    set((s) => ({ directory: setEntryScope(s.directory, peer, scope) })),

  resetConsoleState: () => {
    closeConnection();
    set({
      phase: "idle",
      activePeer: null,
      closeInfo: null,
      reconnect: null,
      reattachNotice: null,
      sessionLostNotice: false,
      capabilities: null,
      sessions: [],
      activeSessionId: null,
      transcripts: {},
      interactions: {},
      promptPendingBySession: {},
      promptDrafts: {},
      lastError: null,
    });
  },
}));
