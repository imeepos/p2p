import { ChatPage } from "@/views/chat/chat-page";

// /chat 挂载点（P1 统一会话视图）：双栏混排页；?kind=*（旧 /group /acp
// 重定向落点）由页内聚焦逻辑消化（拍板项 1），P0 kind 整页分支桥随 P1 移除。
export function ChatRoutePage() {
  return <ChatPage />;
}
