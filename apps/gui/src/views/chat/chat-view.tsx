import { ChatPlaceholder } from "@/components/chat/chat-placeholder";
import { PageHeader } from "@/components/page/page-header";

// 聊天页（T30 契约壳）：空态占位；会话列表/气泡/输入条在 T31 视图波接入。
export function ChatView() {
  return (
    <>
      <PageHeader titleKey="chat.title" descriptionKey="chat.description" />
      <ChatPlaceholder />
    </>
  );
}
