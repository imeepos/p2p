import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { ArrowLeft, Bot } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { Button } from "@/components/ui/button";
import { NARROW_CHAT_QUERY, useMediaQuery } from "@/hooks/use-media-query";
import { AgentConversation } from "./agent-conversation";
import { EmptyState } from "@/views/shared/empty-state";

import { AgentEndpointSidebar } from "./agent-endpoint-sidebar";

// ACS1：/agent 独立 agent 会话页。三区 = 左侧端点/会话侧栏 + 中部对话区
// （会话头 + Transcript + PromptComposer，复用 AgentConversation）+ 底部输入。
// 选中态路由化 ?endpoint=<id>；<768 单栏互斥与 /chat 同规则（§2.1）：默认显
// 侧栏，选中切入对话，对话左上返回清选中。
// 深链兜底：存量 agent 深链由 routes/agent-redirect 改道至此。
// ACS2：会话组件随唯一消费点迁入本目录（agent-conversation.tsx，行为零改动）。
export function AgentChatPage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const narrow = useMediaQuery(NARROW_CHAT_QUERY);
  const endpointId = searchParams.get("endpoint");
  const setFocusedEndpoint = useAcpStore((s) => s.setFocusedEndpoint);

  // 聚焦语义：停在本页即取消未读计数，离开（切端点/卸载）
  // 复原聚焦态，agent 回复重新计未读。
  useEffect(() => {
    setFocusedEndpoint(endpointId);
    return () => setFocusedEndpoint(null);
  }, [endpointId, setFocusedEndpoint]);

  // §2.1 单栏互斥：窄屏侧栏与对话互斥呈现；宽屏双区并存
  const showSidebar = !narrow || !endpointId;
  const showConversation = !narrow || !!endpointId;

  return (
    <div data-testid="agent-chat-page" className="flex min-h-0 flex-1">
      {showSidebar ? <AgentEndpointSidebar selectedEndpointId={endpointId} /> : null}
      {showConversation ? (
        <section
          aria-label={t("agentChat.title")}
          data-testid="agent-chat-conversation-pane"
          className="bg-wx-chat flex min-h-0 min-w-0 flex-1 flex-col"
        >
          {narrow && endpointId ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="mr-auto"
              data-testid="agent-chat-back"
              onClick={() => setSearchParams(new URLSearchParams(), { replace: true })}
            >
              <ArrowLeft aria-hidden />
              {t("agentChat.back")}
            </Button>
          ) : null}
          {endpointId ? (
            <AgentConversation key={endpointId} endpointId={endpointId} />
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center p-6">
              <EmptyState
                icon={Bot}
                title={t("agentChat.empty.title")}
                description={t("agentChat.empty.description")}
              />
            </div>
          )}
        </section>
      ) : null}
    </div>
  );
}
