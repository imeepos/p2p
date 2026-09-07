import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { ipc } from "@/lib/ipc";
import type { ChatFriendJson, GroupJson } from "@/lib/ipc-types";
import {
  buildTargets,
  summarizeSend,
  toggleSelected,
  type SendOutcome,
  type ShareTarget,
} from "@/acp/share-targets-model";

/** 已生成链接后的发送目标区：好友与群聊多选，逐个作为普通文本消息发送
 *  （acp-share §8：链接即正文，不改 IM 线协议）。无联系人时整区不渲染。 */
export function ShareSendTargets({ link }: { link: string }) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<ShareTarget[] | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [sending, setSending] = useState(false);
  const [summary, setSummary] = useState<{ sent: number; failed: number } | null>(null);

  useEffect(() => {
    let dead = false;
    void Promise.all([ipc.chatFriendsList().catch(() => [] as ChatFriendJson[]), ipc.groupList().catch(() => [] as GroupJson[])])
      .then(([friends, groups]) => {
        if (dead) return;
        setTargets(buildTargets(friends, groups));
      });
    return () => {
      dead = true;
    };
  }, []);

  if (targets === null || targets.length === 0) return null;
  const selectedCount = selected.length;
  const disabled = sending || selectedCount === 0;

  const send = async () => {
    setSending(true);
    setSummary(null);
    try {
      const outcomes: SendOutcome[] = [];
      for (const key of selected) {
        const target = targets.find((x) => x.key === key);
        if (!target) continue;
        try {
          if (target.kind === "friend") {
            await ipc.chatSend(target.id, "text", link);
          } else {
            await ipc.groupSend(target.id, "text", link);
          }
          outcomes.push({ key, ok: true });
        } catch {
          outcomes.push({ key, ok: false });
        }
      }
      setSummary(summarizeSend(outcomes));
      setSelected([]);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="flex flex-col gap-2" data-testid="acp-share-targets">
      <Label className="text-muted-foreground text-xs">{t("acp.share.targets.label")}</Label>
      <div className="grid max-h-40 grid-cols-1 gap-1 overflow-y-auto sm:grid-cols-2">
        {targets.map((target) => (
          <label
            key={target.key}
            className="flex cursor-pointer items-center gap-2 rounded-md border px-2 py-1.5 text-sm"
            data-testid={"acp-share-target-" + target.key}
          >
            <input
              type="checkbox"
              className="accent-primary size-4"
              checked={selected.includes(target.key)}
              onChange={() => setSelected((prev) => toggleSelected(prev, target.key))}
              aria-label={target.label}
              data-testid={"acp-share-target-check-" + target.key}
            />
            <span className="min-w-0 truncate">{target.label}</span>
            <span className="text-muted-foreground ml-auto shrink-0 text-[10px]">
              {t(target.kind === "friend" ? "acp.share.targets.friend" : "acp.share.targets.group")}
            </span>
          </label>
        ))}
      </div>
      <div className="flex items-center justify-between gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={disabled}
          onClick={() => void send()}
          data-testid="acp-share-send-targets"
        >
          {sending
            ? t("acp.share.targets.sending")
            : t("acp.share.targets.send", { count: selectedCount })}
        </Button>
        {summary ? (
          <span
            className={
              summary.failed === 0
                ? "text-success text-xs"
                : "text-destructive text-xs"
            }
            data-testid="acp-share-targets-summary"
          >
            {summary.failed === 0
              ? t("acp.share.targets.sentAll", { count: summary.sent })
              : t("acp.share.targets.sentPartial", { sent: summary.sent, failed: summary.failed })}
          </span>
        ) : null}
      </div>
    </div>
  );
}

