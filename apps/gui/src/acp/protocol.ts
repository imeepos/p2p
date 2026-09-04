// ACP JSON-RPC 面类型与线级常量（docs/design/acp-over-p2p-design.md §4/§8）。
// GUI 直面本地 WS：acp-console 是哑泵，这里的方法面即 ACP 方法面（约 10 个）。
// 本文件属协议层（非 UI），禁止引入 React/i18n 依赖。

/** acp-console WS 关闭码（apps/acp-console/README.md）：握手拒绝 / 拨号失败 */
export const CLOSE_DENIED = 4403;
export const CLOSE_DIAL_FAILED = 4500;

/** GUI 视角的连接阶段；console 侧 reattach-window 折射为 reconnecting 提示 */
export type AcpPhase = "idle" | "connecting" | "online" | "reconnecting" | "offline";

export interface AcpEndpoint {
  /** 本地 WS 地址，形如 ws://127.0.0.1:<port> */
  wsUrl: string;
  /** console 鉴权 token（必填，错 token 在升级层 401/4403 拒绝） */
  token: string;
  /** 目标 agent 节点 PeerId（base58） */
  peer: string;
}

export interface AcpContentBlock {
  type: "text";
  text: string;
}

export interface AgentCapabilities {
  loadSession?: boolean;
  promptCapabilities?: { embeddedContext?: boolean };
}

export interface InitializeResult {
  protocolVersion?: number | string;
  agentInfo?: { name?: string; version?: string };
  agentCapabilities?: AgentCapabilities;
}

export interface SessionSummary {
  sessionId: string;
  title?: string;
  cwd?: string;
}

export interface SessionListResult {
  sessions?: SessionSummary[];
}

export interface SessionNewResult {
  sessionId: string;
}

export interface PromptResult {
  stopReason?: string;
}

export interface JsonRpcErrorShape {
  code: number;
  message: string;
}

export type SessionUpdate =
  | { sessionUpdate: "agent_message_chunk"; content?: AcpContentBlock }
  | { sessionUpdate: "agent_thought_chunk"; content?: AcpContentBlock }
  | { sessionUpdate: string; [extra: string]: unknown };

export interface SessionUpdateParams {
  sessionId?: string;
  update?: SessionUpdate;
}

/** GUI 侧 initialize 参数：clientCapabilities 如实申报，不虚报文件系能力 */
export function initializeParams(): Record<string, unknown> {
  return {
    protocolVersion: 1,
    clientCapabilities: { fs: { readTextFile: false, writeTextFile: false } },
  };
}

export function buildWsUrl(endpoint: AcpEndpoint): string {
  const url = new URL(endpoint.wsUrl);
  url.searchParams.set("token", endpoint.token);
  url.searchParams.set("peer", endpoint.peer);
  return url.toString();
}

export type AcpCloseKind = "denied" | "dial-failed" | "closed" | "abnormal";

export interface AcpCloseInfo {
  kind: AcpCloseKind;
  code: number;
  reason: string;
}

export function classifyClose(code: number, reason: string): AcpCloseInfo {
  if (code === CLOSE_DENIED) return { kind: "denied", code, reason };
  if (code === CLOSE_DIAL_FAILED) return { kind: "dial-failed", code, reason };
  if (code === 1000) return { kind: "closed", code, reason };
  return { kind: "abnormal", code, reason };
}

export function promptParams(
  sessionId: string,
  text: string,
): Record<string, unknown> {
  const block = { type: "text", text };
  return { sessionId, prompt: [block] };
}
