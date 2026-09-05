import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useAcpStore } from "@/acp/acp-store";

export function PromptComposer() {
  const { t } = useTranslation();
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  // 草稿按会话隔离（P1）：值域挂在 store 的 promptDrafts[activeSessionId] 上，
  // 切会话各显各的草稿，杜绝把话发给另一个 agent
  const text = useAcpStore((s) =>
    s.activeSessionId ? (s.promptDrafts[s.activeSessionId] ?? "") : "",
  );
  const setPromptDraft = useAcpStore((s) => s.setPromptDraft);
  const pending = useAcpStore((s) =>
    s.activeSessionId ? (s.promptPendingBySession[s.activeSessionId] ?? false) : false,
  );
  const sendPrompt = useAcpStore((s) => s.sendPrompt);
  const cancelPrompt = useAcpStore((s) => s.cancelPrompt);

  const submit = () => {
    if (!activeSessionId || pending || !text.trim()) return;
    // 发送失败草稿保留（可原样重发），成功由 store 清空该会话草稿
    void sendPrompt(text);
  };

  return (
    <div className="flex items-end gap-2">
      <Textarea
        value={text}
        onChange={(e) => {
          if (activeSessionId) setPromptDraft(activeSessionId, e.target.value);
        }}
        onKeyDown={(e) => {
          // Enter 发送、Shift+Enter 换行（IME 组合态守卫由后续提交接入）
          if (e.key !== "Enter" || e.shiftKey) return;
          e.preventDefault();
          submit();
        }}
        placeholder={t("acp.composer.placeholder")}
        data-testid="acp-composer-input"
        rows={2}
        className="min-h-0 flex-1 resize-none"
      />
      {pending ? (
        <Button variant="destructive" onClick={cancelPrompt} data-testid="acp-composer-stop">
          {t("acp.composer.stop")}
        </Button>
      ) : (
        <Button onClick={submit} disabled={!text.trim()} data-testid="acp-composer-send">
          {t("acp.composer.send")}
        </Button>
      )}
    </div>
  );
}
