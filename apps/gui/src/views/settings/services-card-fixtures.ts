import type { ServiceView } from "@/lib/ipc-types";

// §20.1 闭集 10 项测试夹具：id/kind 与契约一致，enabled/requiresRestart
// 供用例按需覆写（面板渲染数据源一律来自 servicesList 响应）。
export function fullServiceList(
  overrides?: { enabled?: boolean; requiresRestart?: boolean },
): ServiceView[] {
  return [
    { serviceId: "serve.llm_share", kind: "boolean", enabled: false, requiresRestart: false },
    { serviceId: "serve.tunnel", kind: "boolean", enabled: false, requiresRestart: false },
    { serviceId: "serve.a2a", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "serve.acp", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "net.rendezvous_register", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "net.relay", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "net.observe", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "serve.rendezvous_server", kind: "explicit", enabled: true, requiresRestart: false },
    { serviceId: "discovery.mdns", kind: "adopted", enabled: true, requiresRestart: false },
    { serviceId: "net.lan_only", kind: "adopted", enabled: false, requiresRestart: false },
  ].map((row) => ({ ...row, ...overrides }));
}
