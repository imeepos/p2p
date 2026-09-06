// /llm-share 挂载点（对齐 docs-page 模式）：实现驻 views/llm-share。
// 数据接缝按 VITE_MOCK_IPC 选择 mock/live（契约 §16.1 命令名逐字）；
// lib/** 归 LSG2，其 IPC 面落地后仅此处的 backend 接线需要跟进。
import { resolveLlmShareBackend } from "@/views/llm-share/backend";
import { LlmShareView } from "@/views/llm-share/llm-share-view";

export function LlmSharePage() {
  return <LlmShareView backend={resolveLlmShareBackend()} />;
}
