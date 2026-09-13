// 首用零手填（2026-09-14 用户裁决：连接缺 token/peer 先自动补全，不把配置难度丢给用户）：
// token/wsUrl/statusUrl 取 console 状态快照（pump 启动随机生成一次、经状态面分发），
// peer 取本机 agent 自描述（~/.dsh/acp/local-agent.json 持久身份）。只填空白字段，
// 绝不覆盖用户手填值；IPC 面缺失（旧 webview/浏览器直开）或不可达时告警留痕原样返回。
import { ipc } from "@/lib/ipc";

import type { AcpEndpoint } from "./protocol";

function faceReady(fn: unknown): boolean {
  return typeof fn === "function";
}

export interface AutofillOutcome {
  next: AcpEndpoint;
  /** 是否有字段被补上（调用方据此回写草稿表单并存档） */
  changed: boolean;
}

/** 连接前置自动补全：complete 草稿零 IPC 直通；缺 token/peer 才逐面探测 */
export async function autofillDraft(draft: AcpEndpoint): Promise<AutofillOutcome> {
  if (draft.token && draft.peer) return { next: draft, changed: false };
  let next = draft;
  if (!next.token && faceReady(ipc.acpConsoleStatus)) {
    try {
      const status = await ipc.acpConsoleStatus();
      if (status?.phase === "connected" && status.token) {
        next = {
          ...next,
          wsUrl: next.wsUrl || status.wsUrl || next.wsUrl,
          token: status.token,
          statusUrl: next.statusUrl || status.statusUrl,
        };
      }
    } catch (error) {
      console.warn("[acp] 自动补全：console 状态不可达", error);
    }
  }
  if (!next.peer && faceReady(ipc.acpLocalDescriptor)) {
    try {
      const descriptor = await ipc.acpLocalDescriptor();
      if (descriptor?.peer) next = { ...next, peer: descriptor.peer };
    } catch (error) {
      console.warn("[acp] 自动补全：本机 agent 描述不可达", error);
    }
  }
  return { next, changed: next !== draft };
}

/** 自动补全回写草稿的差异面：只带回变化的键，setDraft 合并时不动用户并发输入 */
export function changedFields(before: AcpEndpoint, next: AcpEndpoint): Partial<AcpEndpoint> {
  const patch: Partial<AcpEndpoint> = {};
  if (before.wsUrl !== next.wsUrl) patch.wsUrl = next.wsUrl;
  if (before.token !== next.token) patch.token = next.token;
  if (before.peer !== next.peer) patch.peer = next.peer;
  if ((before.statusUrl ?? "") !== (next.statusUrl ?? "")) patch.statusUrl = next.statusUrl;
  return patch;
}
