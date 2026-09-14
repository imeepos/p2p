import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useAcpStore } from "@/acp/acp-store";
import { isImeComposing } from "@/acp/ime-guard";

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
    // uix-spec #21：输入卡——22px 圆角、0.5px 描边、soft 阴影、卡内 textarea，
    // 与消息列共用内容宽度轴（--dsh-chat-content-width），钉在会话区底部
    <div className="shrink-0 px-4 pb-3">
      <div className="mx-auto flex w-full max-w-[var(--dsh-chat-content-width)] items-end gap-2 rounded-[22px] border-[0.5px] border-border bg-card px-3 py-2 shadow-sm">
        <Textarea
          value={text}
          onChange={(e) => {
            if (activeSessionId) setPromptDraft(activeSessionId, e.target.value);
          }}
          onKeyDown={(e) => {
            // Enter 发送、Shift+Enter 换行；IME 组合态（含 keyCode 229 兜底）
            // 的 Enter 是选词确认，不得触发发送（P0）
            if (e.key !== "Enter" || e.shiftKey) return;
            if (isImeComposing(e.nativeEvent)) return;
            e.preventDefault();
            submit();
          }}
          placeholder={t("acp.composer.placeholder")}
          data-testid="acp-composer-input"
          rows={1}
          className="max-h-[336px] min-h-9 flex-1 resize-none border-0 bg-transparent px-1 py-1.5 shadow-none focus-visible:border-transparent focus-visible:ring-0 dark:bg-transparent"
        />
        {/* 回合进行中（AG-UI RUN_STARTED 窗口）发送禁用防重复提交，结算解除；
            Stop 仅进行中出现，保留取消入口 */}
        <div className="flex shrink-0 items-center gap-1.5 pb-0.5">
          {pending ? (
            <Button
              variant="destructive"
              size="sm"
              className="rounded-full px-4"
              onClick={cancelPrompt}
              data-testid="acp-composer-stop"
            >
              {t("acp.composer.stop")}
            </Button>
          ) : null}
          <Button
            size="sm"
            className="rounded-full px-4"
            onClick={submit}
            disabled={pending || !text.trim()}
            data-testid="acp-composer-send"
          >
            {t("acp.composer.send")}
          </Button>
        </div>
      </div>
    </div>
  );
}
