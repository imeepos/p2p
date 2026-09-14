import { Navigate, useLocation } from "react-router-dom";

import { ChatPage } from "@/views/chat/chat-page";
import { agentRedirectTarget } from "./agent-redirect";

// /chat 挂载点（P1 统一会话视图）：双栏混排页；?kind=*（旧 /group /acp
// 重定向落点）由页内聚焦逻辑消化（拍板项 1），P0 kind 整页分支桥随 P1 移除。
// ACS1：agent 深链（?agent=/?kind=agent）先改道独立页 /agent 兜底；未命中一律
// 原样交给 ChatPage——页内 agent 条目与形态不动（拆除属 ACS2）。
export function ChatRoutePage() {
  const { search } = useLocation();
  const target = agentRedirectTarget(search);
  if (target) return <Navigate to={target} replace />;
  return <ChatPage />;
}
