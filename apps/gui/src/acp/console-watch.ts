// pump 进程内状态编排（契约 §15，INLINE-ACP-PUMP 起为 in-process 装配）：订阅
// acp-console 状态面，connected 即幂等登记「本机 agent」端点并自动连接、自动开
// 新会话（/chat?agent=<本机id> 零二次点击直落）。peer 不在契约 status 内，解析
// 顺序：① 本机 agent 自描述文件（~/.dsh/acp/local-agent.json，持久身份，开箱
// 即连主路径）→ ② console 发现面轮询（首条发现条目，agent 从未落盘时回落）；
// 都解析不到显式留痕挂起，绝不静默、绝不连接风暴。
import { ipc } from "@/lib/ipc";
import type { AcpConsoleStatus } from "@/lib/ipc-types";
import i18n from "@/i18n";

import {
  LOCAL_AGENT_ENDPOINT_ID,
  fetchDiscoveryPeers,
  localAgentEndpointOf,
  mergeLocalAgent,
  stampLocalPeer,
} from "./console-client";
import { fetchLocalAgentPeer } from "./local-agent-peer";
import { persistStored } from "./endpoint-storage";
import type { AcpEndpoint } from "./protocol";
import { useAcpStore } from "./acp-store";

// 自动流程舞台：相位重放/状态重订阅下的幂等闸门
type FlowStage = "idle" | "registered" | "connecting" | "opening" | "done";

let stage: FlowStage = "idle";
let watching = false;
let unsubscribe: (() => void) | null = null;
let pollTimer: ReturnType<typeof setTimeout> | null = null;
let pollAttempts = 0;
let lastAutoKey = "";
let descriptorInFlight = false;

const DISCOVERY_POLL_MS = 1_000;
const DISCOVERY_MAX_ATTEMPTS = 10;

function get(): ReturnType<typeof useAcpStore.getState> {
  return useAcpStore.getState();
}

function setNotice(notice: string | null): void {
  useAcpStore.setState({ consoleFlowNotice: notice });
}

function stopDiscoveryPoll(): void {
  if (pollTimer !== null) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
}

/** 幂等登记进收藏（绕过 upsertSaved：不污染用户草稿，只动 saved 档） */
function registerLocalAgent(changed: boolean, saved: AcpEndpoint[]): void {
  if (!changed) return;
  useAcpStore.setState({ saved });
  persistStored({ draft: get().draft, saved });
}

function autoKeyOf(status: AcpConsoleStatus): string {
  return [status.phase, status.wsUrl ?? "", status.token ?? ""].join("|");
}

/** 单连接语义守卫：已连其他 agent 不抢占；已在本机 agent 上不重复拨 */
function canAutoConnect(endpoint: AcpEndpoint): boolean {
  const s = get();
  if (!endpoint.peer) return false;
  if (s.phase === "online" || s.phase === "connecting" || s.phase === "reconnecting") {
    if (s.activeEndpointId === LOCAL_AGENT_ENDPOINT_ID) return false;
    console.warn("[acp] 已连接其他 agent，跳过本机 agent 自动连接（端点已登记）");
    setNotice("acp.console.connectSkipped");
    stage = "done";
    return false;
  }
  return true;
}

function startAutoConnect(endpoint: AcpEndpoint): void {
  if (!canAutoConnect(endpoint)) return;
  stage = "connecting";
  const s = get();
  s.setDraft(endpoint);
  s.connect();
}

/** 发现面解析 peer（首条发现条目 = 本机 agent）并落端点后自动连接 */
function resolvePeerFromDirectory(): boolean {
  const s = get();
  // 已解析过（存量端点带 peer）直接跳过：防订阅重触发下的 setState 回环
  if (s.saved.some((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID && e.peer)) return false;
  const discovered = s.directory.find((e) => e.source === "discovered");
  const draft = localAgentEndpointOf(s.console);
  if (!discovered || !draft) return false;
  // mergeLocalAgent 保留用户 peer（本处为空才解析）：解析结果显式回写存档
  const stamped = stampLocalPeer(
    s.saved,
    draft,
    discovered.peer,
    i18n.t("acp.console.localAgentName"),
  );
  useAcpStore.setState({ saved: stamped.saved });
  persistStored({ draft: get().draft, saved: stamped.saved });
  setNotice(null);
  startAutoConnect(stamped.endpoint);
  return true;
}

/** 描述文件解析 peer（开箱主路径）：stamp 进存档并自动连接；不可用返回 false 回落发现面 */
async function resolvePeerFromDescriptor(): Promise<boolean> {
  if (descriptorInFlight) return false;
  descriptorInFlight = true;
  try {
    const peer = await fetchLocalAgentPeer();
    const s = get();
    if (
      !peer ||
      s.console?.phase !== "connected" ||
      s.saved.some((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID && e.peer)
    ) {
      return false;
    }
    const draft = localAgentEndpointOf(s.console);
    if (!draft) return false;
    const stamped = stampLocalPeer(s.saved, draft, peer, i18n.t("acp.console.localAgentName"));
    useAcpStore.setState({ saved: stamped.saved });
    persistStored({ draft: get().draft, saved: stamped.saved });
    setNotice(null);
    startAutoConnect(stamped.endpoint);
    return true;
  } finally {
    descriptorInFlight = false;
  }
}

/** connected 后 peer 缺失的解析编排：描述文件优先，失败回落发现面轮询 */
function resolvePeerFallbackToDiscovery(status: AcpConsoleStatus): void {
  void resolvePeerFromDescriptor().then((resolved) => {
    if (resolved || stage !== "registered") return;
    if (get().console?.phase !== "connected") return;
    if (status.statusUrl && status.token) {
      startDiscoveryPoll(status.statusUrl, status.token);
    } else {
      console.warn("[acp] connected 快照缺 statusUrl：无法解析本机 agent peer，自动流程挂起");
      setNotice("acp.console.resolveFailed");
    }
  });
}

function startDiscoveryPoll(statusUrl: string, token: string): void {
  stopDiscoveryPoll();
  pollAttempts = 0;
  setNotice("acp.console.waitingDiscovery");
  const tick = async (): Promise<void> => {
    pollAttempts += 1;
    const peers = await fetchDiscoveryPeers(statusUrl, token);
    if (get().console?.phase !== "connected") {
      stopDiscoveryPoll();
      return;
    }
    if (peers !== null && peers.length > 0) {
      get().ingestDiscovery(peers);
    }
    if (resolvePeerFromDirectory()) {
      stopDiscoveryPoll();
      return;
    }
    if (peers === null) {
      console.warn("[acp] console 发现面不可达，本机 agent 节点定位停止");
      setNotice("acp.console.resolveFailed");
      stopDiscoveryPoll();
      return;
    }
    if (pollAttempts >= DISCOVERY_MAX_ATTEMPTS) {
      console.warn("[acp] 发现面多轮为空：本机 agent 节点未出现，自动流程挂起");
      setNotice("acp.console.resolveFailed");
      stopDiscoveryPoll();
      return;
    }
    pollTimer = setTimeout(() => void tick(), DISCOVERY_POLL_MS);
  };
  void tick();
}

function applyConsoleStatus(status: AcpConsoleStatus): void {
  useAcpStore.setState({ console: status });
  if (status.phase !== "connected") {
    stopDiscoveryPoll();
    // 断开相位复位舞台：下一次 connected（泵重启成功）重新走登记+连接
    if (status.phase === "disconnected") {
      stage = "idle";
      lastAutoKey = "";
    }
    return;
  }
  const key = autoKeyOf(status);
  // 相位重放幂等闸：同 ready 快照且流程已在途/完成时不重复登记连接
  if (key === lastAutoKey && stage !== "idle") return;
  lastAutoKey = key;
  const draft = localAgentEndpointOf(status);
  if (!draft) {
    console.warn("[acp] connected 快照缺连接面（wsUrl/token），本机 agent 登记挂起");
    return;
  }
  const merge = mergeLocalAgent(get().saved, draft, i18n.t("acp.console.localAgentName"));
  // 重放：登记未变化且流程已完成，直接收敛
  if (stage === "done" && !merge.changed) return;
  registerLocalAgent(merge.changed, merge.saved);
  stage = "registered";
  if (merge.endpoint.peer) {
    startAutoConnect(merge.endpoint);
    return;
  }
  resolvePeerFallbackToDiscovery(status);
}

function ensureStoreSubscription(): void {
  if (unsubscribe) return;
  unsubscribe = useAcpStore.subscribe((s, prev) => {
    // 自动开新会话：握手（initialize）完成且尚无活动会话
    if (
      stage === "connecting" &&
      s.phase === "online" &&
      s.activeEndpointId === LOCAL_AGENT_ENDPOINT_ID &&
      s.capabilities !== null
    ) {
      if (s.activeSessionId) {
        stage = "done";
        return;
      }
      stage = "opening";
      void get()
        .newSession()
        .catch(() => setNotice("acp.console.sessionOpenFailed"))
        .finally(() => {
          stage = "done";
        });
      return;
    }
    // 发现面到达（边沿触发：仅 directory 变化时解析一次，防 setState 回环）
    if (stage === "registered" && s.directory !== prev.directory) {
      resolvePeerFromDirectory();
    }
  });
}

/** 幂等启动：订阅相位事件 + 取一次状态快照。IPC 面缺失（UX2 未合入/旧后端）
 *  显式告警并退出，不静默吞。 */
export function ensureConsoleWatch(): void {
  if (watching) return;
  if (typeof ipc.acpConsoleStatus !== "function" || typeof ipc.onAcpConsoleEvent !== "function") {
    console.warn("[acp] IPC 面缺 acp_console_status：console 托管能力未就绪");
    return;
  }
  watching = true;
  ensureStoreSubscription();
  void ipc
    .onAcpConsoleEvent(applyConsoleStatus)
    .then((unlistenEvent) => {
      const prev = unsubscribe;
      unsubscribe = () => {
        unlistenEvent();
        prev?.();
      };
    })
    .catch((error) => console.warn("[acp] acp-console 事件订阅失败", error));
  ipc
    .acpConsoleStatus()
    .then(applyConsoleStatus)
    .catch((error) => console.warn("[acp] acp_console_status 调用失败（伴生托管未就绪）", error));
}

// 生产/开发随模块加载即启动（chat 路由静态 import 链保证应用启动即求值）；
// 测试环境不自动启动，由用例显式调用 ensureConsoleWatch，避免副作用污染用例。
if (import.meta.env.MODE !== "test") {
  ensureConsoleWatch();
}

/** 测试隔离入口：复位模块级舞台与轮询（store 状态由用例自行 resetConsoleState） */
export function resetConsoleWatchForTest(): void {
  stopDiscoveryPoll();
  stage = "idle";
  lastAutoKey = "";
  watching = false;
  descriptorInFlight = false;
  unsubscribe?.();
  unsubscribe = null;
}
