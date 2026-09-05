import { useLocation } from "react-router-dom";

import { AcpPage } from "@/routes/acp-page";
import { ChatPage } from "@/routes/chat-page";
import { GroupPage } from "@/routes/group-page";

// /chat 挂载点（七、迁移期并存策略）：/group /acp 旧路由重定向到
// /chat?kind=* 后，群聊/ACP 视图在 P1 双栏混排落地前保持整页形态挂新壳内，
// 经 kind 参数选择；P1 统一会话视图就位后移除本分支。
export function ChatRoutePage() {
  const kind = new URLSearchParams(useLocation().search).get("kind");
  if (kind === "group") return <GroupPage />;
  if (kind === "agent") return <AcpPage />;
  return <ChatPage />;
}
