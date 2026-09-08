// 发现视图（设计 §8.2）：公开智能体节。行 = 头像(Bot)+名称+行内[聊天][详情] /
// 描述截断+skills 徽章 / monospace 宿主 peer+copy+在线点+可见性徽章；
// 空态 EmptyState 引导到「我的」创建。聊天为 A2A4 交付面（占位 toast 显式说明）。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Bot, MessageCircle, Info } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { toastInfo } from "@/components/feedback/toast";
import { SectionHeader } from "@/views/shared/section-header";
import { cn } from "@/lib/utils";

import type { AgentCardJson, DiscoveredAgent } from "@/a2a/types";

import { HostPeerCell, onlineLevel } from "./agent-bits";

function SkillsBadges({ card }: { card: AgentCardJson }) {
  if (!card.skills || card.skills.length === 0) return null;
  return (
    <span className="flex flex-wrap gap-1">
      {card.skills.slice(0, 4).map((skill) => (
        <span
          key={skill.id}
          className="bg-muted text-muted-foreground rounded px-1.5 py-0.5 text-[10px] leading-4"
        >
          {skill.name}
        </span>
      ))}
    </span>
  );
}

export function DiscoverSection({ rows }: { rows: DiscoveredAgent[] }) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<AgentCardJson | null>(null);
  return (
    <section data-testid="agents-discover-section" className="flex flex-col gap-2">
      <SectionHeader icon={Bot} title={t("agents.section.public")} tone="agents" count={rows.length} />
      {rows.length === 0 ? (
        <p className="text-muted-foreground rounded-md border border-dashed p-4 text-center text-sm" data-testid="agents-discover-empty">
          {t("agents.empty.discover")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {rows.map((row) => {
            const card = row.card;
            return (
              <li
                key={card.hostPeer + "/" + card.agentId}
                data-testid="agents-discover-row"
                className="bg-card ring-border flex flex-col gap-1 rounded-lg p-3 ring-1"
              >
                <div className="flex items-center gap-2">
                  <span className="bg-violet-600/15 text-violet-600 flex size-7 shrink-0 items-center justify-center rounded-md">
                    <Bot aria-hidden className="size-4" />
                  </span>
                  <span className="text-sm font-medium">{card.name}</span>
                  <span className="ml-auto flex items-center gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="gap-1 px-2"
                      data-testid="agents-chat-btn"
                      onClick={() => toastInfo(t("agents.toast.chatNotReady"))}
                    >
                      <MessageCircle aria-hidden className="size-4" />
                      {t("agents.action.chat")}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="gap-1 px-2"
                      data-testid="agents-detail-btn"
                      onClick={() => setDetail(card)}
                    >
                      <Info aria-hidden className="size-4" />
                      {t("agents.action.detail")}
                    </Button>
                  </span>
                </div>
                <p className="text-muted-foreground truncate text-sm">{card.description}</p>
                <SkillsBadges card={card} />
                <HostPeerCell card={card} level={onlineLevel(Date.now() / 1000, row)} />
              </li>
            );
          })}
        </ul>
      )}
      <Dialog open={detail !== null} onOpenChange={(o) => !o && setDetail(null)}>
        <DialogContent className="sm:max-w-sm" data-testid="agents-detail-dialog">
          <DialogHeader>
            <DialogTitle>{detail?.name ?? ""}</DialogTitle>
            <DialogDescription>{detail?.description ?? ""}</DialogDescription>
          </DialogHeader>
          {detail ? (
            <dl className="text-muted-foreground flex flex-col gap-1 text-xs">
              <div className="flex justify-between gap-2">
                <dt>agentId</dt>
                <dd className="text-foreground font-mono">{detail.agentId}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt>{t("agents.detail.url")}</dt>
                <dd className={cn("text-foreground truncate font-mono")}>{detail.url}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt>{t("agents.detail.skills")}</dt>
                <dd className="text-foreground">
                  {detail.skills?.map((s) => s.name).join("、") || t("agents.detail.none")}
                </dd>
              </div>
            </dl>
          ) : null}
        </DialogContent>
      </Dialog>
    </section>
  );
}
