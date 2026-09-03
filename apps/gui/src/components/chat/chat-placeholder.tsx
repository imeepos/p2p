import { MessageCircle } from "lucide-react";
import { useTranslation } from "react-i18next";

import { EmptyState } from "@/views/shared/empty-state";

// T30 契约壳占位：好友簿/消息能力已在 IPC 层就绪，会话视图在 T31 接入后移除。
export function ChatPlaceholder() {
  const { t } = useTranslation();

  return (
    <EmptyState
      className="col-span-12 min-h-72"
      icon={MessageCircle}
      title={t("chat.placeholder.title")}
      description={t("chat.placeholder.hint")}
    />
  );
}
