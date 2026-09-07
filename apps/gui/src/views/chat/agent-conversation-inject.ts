import type { PermissionOption } from "@/acp/protocol";
import { LOCAL_AGENT_ENDPOINT_ID } from "@/acp/console-client";
import {
  addPermission,
  emptyInteraction,
} from "@/acp/interaction-model";
import { useAcpStore } from "@/acp/acp-store";

// R2-24 测试注入路径：mock 回放脚本默认无权限步，待应答态无法自然触达；
// 仅 VITE_MOCK_IPC=1 下在 window 挂注入入口（登记形状与 store-events 消费
// request_permission 帧同形），供页面走查以注入态验证指示条与深链。
// 生产构建不暴露。注入不新增交互范式，仅填 store 既有状态。
type AcpTestInjectWindow = {
  __acpInjectPendingPermission?: (sessionId?: string) => void;
  __acpInjectAgentOnlineSession?: (endpointId?: string) => void;
};
if (import.meta.env.VITE_MOCK_IPC === "1" && typeof window !== "undefined") {
  (window as unknown as AcpTestInjectWindow).__acpInjectPendingPermission = (sessionId?: string) => {
    const state = useAcpStore.getState();
    const sid = sessionId ?? state.activeSessionId;
    if (!sid) {
      console.warn("[acp] 注入待应答权限需要活动会话（activeSessionId 为空）");
      return;
    }
    const options: PermissionOption[] = [
      { optionId: "allow-once", name: "Allow", kind: "allow_once" },
      { optionId: "reject-once", name: "Deny", kind: "reject_once" },
    ];
    const next = addPermission(state.interactions[sid] ?? emptyInteraction(), {
      requestId: Math.floor(Math.random() * 1_000_000_000),
      sessionId: sid,
      // 注入数据为 mock 合成帧载荷（同 mock-script 英文惯例），非产品文案
      title: "Execute command (injected for walkthrough)",
      toolKind: "execute",
      options,
      receivedAt: Date.now(),
    });
    useAcpStore.setState({
      interactions: { ...state.interactions, [sid]: next },
    });
  };
  // 在线会话态注入：浏览器 mock 无 console 伴生进程（真机 8787 与 mock
  // token 不一致），连接在线态无法自然触达；仅置 store 既有状态供走查。
  (window as unknown as AcpTestInjectWindow).__acpInjectAgentOnlineSession = (
    endpointId?: string,
  ) => {
    const ep = endpointId ?? LOCAL_AGENT_ENDPOINT_ID;
    useAcpStore.setState({
      phase: "online",
      activeEndpointId: ep,
      activeSessionId: useAcpStore.getState().activeSessionId ?? "s-test-inject",
    });
  };
}
