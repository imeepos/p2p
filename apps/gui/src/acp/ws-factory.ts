// WS 客户端工厂：mock 模式（VITE_MOCK_IPC=1）切换为可脚本化 mock 套接字，
// 测试经 setWsFactory 注缝。与 lib/ipc.ts 同款：mock 仅动态加载避免进 prod bundle。
import { createMockWsFactory } from "./mock-acp-ws";

/** 与浏览器 WebSocket 事件面同形的最小接口，便于 mock 与测试替身 */
export interface WsLike {
  send(data: string): void;
  close(code?: number, reason?: string): void;
  onopen: (() => void) | null;
  onclose: ((ev: { code: number; reason: string }) => void) | null;
  onerror: ((ev: { message?: string }) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
}

export type WebSocketFactory = (url: string) => WsLike;

let override: WebSocketFactory | null = null;

/** 测试注缝；传 null 还原默认 */
export function setWsFactory(factory: WebSocketFactory | null): void {
  override = factory;
}

function realFactory(url: string): WsLike {
  // 浏览器 WebSocket 与 WsLike 结构兼容，事件对象形状更宽，此处收窄视角
  return new WebSocket(url) as unknown as WsLike;
}

const useMock = import.meta.env.VITE_MOCK_IPC === "1";

export const mockWsEnabled = useMock;

/** mock 工厂在模块求值期动态加载（与 lib/ipc.ts 的顶层 await 同款） */
const mockFactory = useMock ? createMockWsFactory() : null;

export function resolveWsFactory(): WebSocketFactory {
  if (override) return override;
  if (mockFactory) return mockFactory;
  return realFactory;
}
