// ACP JSON-RPC 面类型与线级常量（docs/design/acp-over-p2p-design.md §4/§8）。
// GUI 直面本地 WS：acp-console 是哑泵，这里的方法面即 ACP 方法面（约 10 个）。
// 本文件属协议层（非 UI），禁止引入 React/i18n 依赖。

/** acp-console WS 关闭码（apps/acp-console/README.md）：握手拒绝 / 拨号失败 */
export const CLOSE_DENIED = 4403;
export const CLOSE_DIAL_FAILED = 4500;

/** 桥侧权限默认超时（apps/acp-agent/README.md：--permission-timeout-secs 默认 60） */
export const PERMISSION_TIMEOUT_MS = 60_000;

/** GUI 视角的连接阶段；console 侧 reattach-window 折射为 reconnecting 提示 */
export type AcpPhase = "idle" | "connecting" | "online" | "reconnecting" | "offline";

/** 续连补放通知（桥约定，apps/acp-agent/README.md 续连流程第 5 步） */
export const REATTACH_METHOD = "dsh/bridge/reattach";

export interface AcpEndpoint {
  /** 本地 WS 地址，形如 ws://127.0.0.1:<port> */
  wsUrl: string;
  /** console 鉴权 token（必填，错 token 在升级层 401/4403 拒绝） */
  token: string;
  /** 目标 agent 节点 PeerId（base58） */
  peer: string;
  /** 续连票据（可选；断线窗口内重连时透传给桥） */
  reattach?: string;
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
  configOptions?: ConfigOption[];
}

/** session/set_config_option 响应：agent 永远回完整配置态（ACP 会话配置契约） */
export interface ConfigOptionsResult {
  configOptions?: ConfigOption[];
}

export interface ConfigOptionChoice {
  value: string;
  name: string;
  description?: string;
}

export interface ConfigOption {
  id: string;
  name: string;
  description?: string;
  category?: string;
  type: "select" | "boolean" | string;
  currentValue: string | boolean;
  options?: ConfigOptionChoice[];
}

export interface UsageUpdate {
  used?: number;
  size?: number;
  cost?: { amount: number; currency: string } | null;
}

export type ToolCallStatus = "pending" | "in_progress" | "completed" | "failed";

export interface ToolCallPayload {
  toolCallId: string;
  title?: string;
  kind?: string;
  status?: ToolCallStatus;
  content?: unknown;
  rawInput?: unknown;
  rawOutput?: unknown;
}

export interface PermissionOption {
  optionId: string;
  name: string;
  kind: "allow_once" | "allow_always" | "reject_once" | "reject_always" | string;
}

/** agent 侧发来的 request_permission 请求参数（ACP 工具权限） */
export interface PermissionRequestParams {
  sessionId?: string;
  toolCall?: { toolCallId?: string; title?: string; kind?: string };
  options?: PermissionOption[];
}

export interface PermissionOutcomeResult {
  outcome: { outcome: "selected"; optionId: string } | { outcome: "cancelled" };
}

export function selectedOutcome(optionId: string): PermissionOutcomeResult {
  return { outcome: { outcome: "selected", optionId } };
}

export function cancelledOutcome(): PermissionOutcomeResult {
  return { outcome: { outcome: "cancelled" } };
}

export interface ReattachParams {
  replayed?: number;
}

export interface PromptResult {
  stopReason?: string;
}

/** ACP v1 已定义的 stopReason；未知值原样透传显示 */
export const PROMPT_STOP_REASONS = [
  "end_turn",
  "max_tokens",
  "max_turn_requests",
  "refusal",
  "cancelled",
] as const;

export type PromptStopReason = (typeof PROMPT_STOP_REASONS)[number];

export type SessionUpdate =
  | { sessionUpdate: "agent_message_chunk"; content?: AcpContentBlock }
  | { sessionUpdate: "agent_thought_chunk"; content?: AcpContentBlock }
  | ({ sessionUpdate: "tool_call" } & ToolCallPayload)
  | ({ sessionUpdate: "tool_call_update"; toolCallId: string } & Partial<ToolCallPayload>)
  | { sessionUpdate: "config_option_update"; configOptions?: ConfigOption[] }
  | ({ sessionUpdate: "usage_update" } & UsageUpdate)
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
  if (endpoint.reattach) url.searchParams.set("reattach", endpoint.reattach);
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