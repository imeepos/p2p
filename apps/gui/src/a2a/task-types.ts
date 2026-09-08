// A2A 任务相关类型（docs/design/a2a-over-p2p-design.md §5.2/§5.3）。
// 真值源 = crates/a2a/src/task.rs serde 输出。

export type A2aTaskState = 
  | "submitted"
  | "working"
  | "completed"
  | "failed"
  | "cancelled"
  | "rejected";

export type A2aPartType = "text" | "file" | "data";

export interface A2aTextPart {
  type: "text";
  text: string;
}

export interface A2aFilePart {
  type: "file";
  name: string;
  mimeType: string;
  bytes: string; // base64
}

export interface A2aDataPart {
  type: "data";
  data: unknown;
}

export type A2aPart = A2aTextPart | A2aFilePart | A2aDataPart;

export interface A2aMessage {
  role: "user" | "agent";
  parts: A2aPart[];
  messageId?: string;
}

export interface A2aTask {
  taskId: string;
  agentId: string;
  state: A2aTaskState;
  messages: A2aMessage[];
}

export interface A2aTaskStatusNotice {
  taskId: string;
  state: A2aTaskState;
}

export interface A2aTaskMessageNotice {
  taskId: string;
  messageId: string;
  message: A2aMessage;
}

export interface A2aTaskSnapshot {
  taskId: string;
  agentId: string;
  state: A2aTaskState;
  messages: A2aMessage[];
}