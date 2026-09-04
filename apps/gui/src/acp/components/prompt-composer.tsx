import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useAcpStore } from "@/acp/acp-store";

export function PromptComposer() {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const pending = useAcpStore((s) => s.promptPending);
  const sendPrompt = useAcpStore((s) => s.sendPrompt);
  const cancelPrompt = useAcpStore((s) => s.cancelPrompt);

  const submit = () => {
    if (pending || !text.trim()) return;
    void sendPrompt(text);
    setText("");
  };

  return (
    <div className="flex items-end gap-2">
      <Textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
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
