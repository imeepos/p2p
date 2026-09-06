// ACP 控制台 store：状态与一行委托。连接实例持有、事件路由在 store-events，
// 动作实现在 acp-actions；本文件只拼装（设计 §8：会话侧栏与 transcript 跨重连存活）。
import { create } from "zustand";

import type { AcpCloseInfo, AcpEndpoint, AcpPhase, InitializeResult, SessionSummary } from "./protocol";
import type { AcpConsoleStatus } from "@/lib/ipc-types";
import type { InteractionState, PermissionNotice } from "./interaction-model";
import type { AcpScope, DirectoryEntry, DiscoveryPeer } from "./directory-model";
import {
  addManual,
  removeEntry,
  setEntryScope,
  upsertDiscovered,
} from "./directory-model";
import { emptyTranscript, toggleThought, type TranscriptState } from "./transcript-model";
import { loadStored, newEndpointId, persistStored } from "./endpoint-storage";
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
  /** 当前连接对应的本地端点 id（§2.2 主键）；草稿未保存时为 null */
  activeEndpointId: string | null;
  /** /chat?agent= 聚焦的端点 id：null 表示聊天页未停在该 agent 会话 */
  focusedEndpointId: string | null;
  /** §2.2/§2.3：每端点最后交互时间与未读（仅内存态，重启归零） */
  lastInteractionByEndpoint: Record<string, number>;
  unreadByEndpoint: Record<string, number>;
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
  /** 最新到达的权限提醒（seq 自增去重），PermissionNoticeBridge 转 toast */
  permissionNotice: PermissionNotice | null;
  permissionSeq: number;
  /** 契约 v10 §15：acp-console 托管状态快照（console-watch 维护，null=尚无快照） */
  console: AcpConsoleStatus | null;
  /** 本机 agent 自动流程的可观测提示（i18n key；自动连接被跳过/发现面定位失败等） */
  consoleFlowNotice: string | null;
  lastError: string | null;
  /** 聊天页深链落定入口：记录聚焦并清零该端点未读（§2.3 选中清零） */
  setFocusedEndpoint: (endpointId: string | null) => void;
  setPromptDraft: (sessionId: string, text: string) => void;
  setDraft: (patch: Partial<AcpEndpoint>) => void;
  saveDraft: () => void;
  removeSaved: (peer: string) => void;
  /** §3.2 通讯录添加流：endpoint 主键兜底补齐后按 id 收藏（peer 可空） */
  upsertSaved: (endpoint: AcpEndpoint) => AcpEndpoint;
  /** §3.3 危险区：按 endpointId 删除收藏（旧 removeSaved 按 peer 键保留兼容） */
  removeSavedById: (endpointId: string) => void;
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
  activeEndpointId: null,
  focusedEndpointId: null,
  lastInteractionByEndpoint: {},
  unreadByEndpoint: {},
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
  permissionNotice: null,
  permissionSeq: 0,
  console: null,
  consoleFlowNotice: null,
  lastError: null,

  setFocusedEndpoint: (endpointId) => {
    set((s) => {
      if (!endpointId) return { focusedEndpointId: null };
      return {
        focusedEndpointId: endpointId,
        // §2.3 选中清零
        unreadByEndpoint: s.unreadByEndpoint[endpointId]
          ? { ...s.unreadByEndpoint, [endpointId]: 0 }
          : s.unreadByEndpoint,
      };
    });
  },

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

  upsertSaved: (endpoint) => {
    const id = endpoint.endpointId?.trim() || newEndpointId();
    const stamped = { ...endpoint, endpointId: id };
    const saved = get().saved;
    // 同 id 覆盖（编辑保存）；同 wsUrl+peer 幂等（重复添加不产生双条目）
    const idx = saved.findIndex(
      (e) =>
        e.endpointId === id ||
        (e.wsUrl === stamped.wsUrl && (e.peer || "") === (stamped.peer || "")),
    );
    const next =
      idx >= 0 ? saved.map((e, i) => (i === idx ? stamped : e)) : [...saved, stamped];
    set({ saved: next, draft: stamped });
    persistStored({ draft: stamped, saved: next });
    return stamped;
  },

  removeSavedById: (endpointId) => {
    const next = get().saved.filter((e) => e.endpointId !== endpointId);
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
      activeEndpointId: null,
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
      permissionNotice: null,
      permissionSeq: 0,
      console: null,
      consoleFlowNotice: null,
      lastError: null,
    });
  },
}));
