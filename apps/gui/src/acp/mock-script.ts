// mock 回放脚本：prompt 步骤扩展出工具时间线/权限请求/配置/用量交互面。
// 帧形状与 ACP v1 一致（tool_call/config_option_update/usage_update/
// session/request_permission），dev（VITE_MOCK_IPC=1）与测试共用同一实现。
import type {
  ConfigOption,
  InitializeResult,
  PermissionOption,
  SessionUpdate,
} from "./protocol";

export interface MockConsoleConfig {
  token: string;
  peers: string[];
  /** 可拨通但 agent 桥拒绝握手（4403 denied）的 peer（真机对拍 R3b） */
  deniedPeers: string[];
  promptScript: MockPromptStep[];
  openDelayMs: number;
  chunkDelayMs: number;
  /** agent 配置目录（session/new 返回 + config_option_update 覆盖） */
  configOptions: ConfigOption[];
}

export const DEFAULT_MOCK_CONFIG: MockConsoleConfig = {
  token: "mock-token",
  peers: ["mock-peer"],
  deniedPeers: [],
  promptScript: [
    { kind: "thought", text: "thinking through the request" },
    { kind: "message", text: "Hello from the mock agent." },
    { kind: "stop", reason: "end_turn" },
  ],
  openDelayMs: 10,
  chunkDelayMs: 10,
  configOptions: defaultConfigOptions(),
};

export type MockPromptStep =
  | { kind: "thought"; text: string }
  | { kind: "message"; text: string }
  | { kind: "tool_call"; toolCallId: string; title: string; callKind?: string; status?: string; input?: unknown }
  | { kind: "tool_update"; toolCallId: string; status?: string; outputText?: string }
  | { kind: "permission"; toolKind: string; title?: string; options?: PermissionOption[] }
  | { kind: "config"; options: ConfigOption[] }
  | { kind: "usage"; used: number; size: number }
  | { kind: "stop"; reason: string };

export const MOCK_PERMISSION_OPTIONS: PermissionOption[] = [
  { optionId: "allow-once", name: "Allow", kind: "allow_once" },
  { optionId: "reject-once", name: "Deny", kind: "reject_once" },
];

export const MOCK_AGENT_INFO: InitializeResult = {
  protocolVersion: 1,
  agentInfo: { name: "mock-agent", version: "0.1.0" },
  agentCapabilities: {
    loadSession: true,
    promptCapabilities: { embeddedContext: false },
  },
};

/** agent 真实目录：默认模型/思考档两枚 select，测试可整表覆盖 */
export function defaultConfigOptions(): ConfigOption[] {
  return [
    {
      id: "model",
      name: "Model",
      category: "model",
      type: "select",
      currentValue: "mock-model-a",
      options: [
        { value: "mock-model-a", name: "Mock Model A" },
        { value: "mock-model-b", name: "Mock Model B" },
      ],
    },
    {
      id: "thought_level",
      name: "Thought Level",
      category: "thought_level",
      type: "select",
      currentValue: "standard",
      options: [
        { value: "minimal", name: "Minimal" },
        { value: "standard", name: "Standard" },
      ],
    },
  ];
}

export function stepToUpdate(step: MockPromptStep, sessionId: string): { method: string; params: unknown } | null {
  if (step.kind === "tool_call") {
    const update: SessionUpdate = {
      sessionUpdate: "tool_call",
      toolCallId: step.toolCallId,
      title: step.title,
      kind: step.callKind,
      status: step.status ?? "pending",
      rawInput: step.input,
    };
    return { method: "session/update", params: { sessionId, update } };
  }
  if (step.kind === "tool_update") {
    const update: SessionUpdate = { sessionUpdate: "tool_call_update", toolCallId: step.toolCallId };
    if (step.status) (update as { status?: string }).status = step.status;
    if (step.outputText !== undefined) {
      (update as { content?: unknown }).content = [
        { type: "content", content: { type: "text", text: step.outputText } },
      ];
    }
    return { method: "session/update", params: { sessionId, update } };
  }
  if (step.kind === "config") {
    return {
      method: "session/update",
      params: { sessionId, update: { sessionUpdate: "config_option_update", configOptions: step.options } },
    };
  }
  if (step.kind === "usage") {
    return {
      method: "session/update",
      params: { sessionId, update: { sessionUpdate: "usage_update", used: step.used, size: step.size } },
    };
  }
  if (step.kind === "thought") {
    return {
      method: "session/update",
      params: { sessionId, update: { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: step.text } } },
    };
  }
  if (step.kind === "message") {
    return {
      method: "session/update",
      params: { sessionId, update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: step.text } } },
    };
  }
  return null;
}

export function permissionRequestFrame(
  id: number,
  sessionId: string,
  step: Extract<MockPromptStep, { kind: "permission" }>,
): Record<string, unknown> {
  return {
    jsonrpc: "2.0",
    id,
    method: "session/request_permission",
    params: {
      sessionId,
      toolCall: { kind: step.toolKind, title: step.title ?? step.toolKind },
      options: step.options ?? MOCK_PERMISSION_OPTIONS,
    },
  };
}