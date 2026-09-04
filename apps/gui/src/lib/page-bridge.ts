// GC3 页面协议桥：控制通道 /page/* 端点经 Rust eval 调 window.__P2P_PAGES__.request('<json>')，
// 本桥解析请求、驱动 page-registry，并把结果经 control-page-reply 事件（带 requestId）回送。
// 页面不新增视觉元素；安装即常驻（生产通道，非 dev 限定）。
import { emit } from "@tauri-apps/api/event";

import { normalizeRoute } from "./control-bridge";
import { describePage, executePageAction, type PageProtocolError } from "./page-registry";

export const PAGE_REPLY_EVENT = "control-page-reply";

export interface PageBridgeRequest {
  requestId: string;
  kind: "describe" | "execute";
  /** describe 可省略：取当前 hash 路由（与 control-bridge 同一归一化） */
  page?: string;
  action?: string;
  args?: Record<string, unknown>;
}

export interface PageBridgeReply {
  requestId: string;
  ok: boolean;
  data?: unknown;
  error?: PageProtocolError;
}

export interface PageBridge {
  request: (payload: string) => void;
}

declare global {
  interface Window {
    __P2P_PAGES__?: PageBridge;
  }
}

export function installPageBridge(): void {
  if (window.__P2P_PAGES__) return;
  window.__P2P_PAGES__ = {
    request: (payload) => {
      void handleRequest(payload);
    },
  };
}

async function handleRequest(payload: string): Promise<void> {
  let request: PageBridgeRequest;
  try {
    request = JSON.parse(payload) as PageBridgeRequest;
  } catch (error) {
    // 无 requestId 无法回送，只能留本地可观测信号
    console.error("[page-bridge] 请求解析失败", error, payload);
    return;
  }
  const reply = await dispatch(request);
  emit(PAGE_REPLY_EVENT, reply).catch((error: unknown) => {
    console.error("[page-bridge] 回复发送失败", request.requestId, error);
  });
}

async function dispatch(request: PageBridgeRequest): Promise<PageBridgeReply> {
  const { requestId } = request;
  if (request.kind === "describe") {
    const page = request.page ?? normalizeRoute(window.location.hash);
    const result = describePage(page);
    if (isProtocolError(result)) {
      return { requestId, ok: false, error: result };
    }
    return { requestId, ok: true, data: result.descriptor };
  }
  if (request.kind === "execute") {
    if (typeof request.page !== "string" || typeof request.action !== "string") {
      return {
        requestId,
        ok: false,
        error: { code: "INVALID_REQUEST", message: "execute 请求必须携带 page 与 action 字符串" },
      };
    }
    const result = await executePageAction(
      request.page,
      request.action,
      request.args ?? {},
    );
    return result.ok
      ? { requestId, ok: true, data: result.data }
      : { requestId, ok: false, error: result.error };
  }
  return {
    requestId,
    ok: false,
    error: { code: "INVALID_REQUEST", message: `未知请求类型: ${String(request.kind)}` },
  };
}

function isProtocolError(
  value: ReturnType<typeof describePage>,
): value is PageProtocolError {
  return typeof value === "object" && value !== null && "code" in value;
}
