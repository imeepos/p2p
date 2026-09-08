// 我的视图（设计 §8.2）：我发布的节 + 节头右侧「+ 创建」主按钮。行内动作 =
// [聊天(自测)][编辑][分享(生成邀请)][下架]；下架走 AsyncButton（失败原文上浮），
// 聊天（A2A4）与分享（A2A5）为占位 toast 显式说明。
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Bot, MessageCircle, Pencil, Share2, Plus, CloudOff } from "lucide-react";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import { toastInfo } from "@/components/feedback/toast";
import { SectionHeader } from "@/views/shared/section-header";

import type { AgentDefJson } from "@/a2a/types";

import { VisibilityBadge } from "./agent-bits";

interface MineSectionProps {
  mine: AgentDefJson[];
  onEdit: (def: AgentDefJson) => void;
  onCreate: () => void;
  onUnpublish: (def: AgentDefJson) => Promise<unknown>;
}

export function MineSection({ mine, onEdit, onCreate, onUnpublish }: MineSectionProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <section data-testid="agents-mine-section" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <SectionHeader icon={Bot} title={t("agents.section.mine")} tone="primary" count={mine.length} />
        <Button type="button" size="sm" onClick={onCreate} data-testid="agents-create-btn">
          <Plus aria-hidden className="size-4" />
          {t("agents.action.create")}
        </Button>
      </div>
      {mine.length === 0 ? (
        <p className="text-muted-foreground rounded-md border border-dashed p-4 text-center text-sm" data-testid="agents-mine-empty">
          {t("agents.empty.mine")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {mine.map((def) => (
            <li
              key={def.agentId}
              data-testid="agents-mine-row"
              className="bg-card ring-border flex flex-col gap-1 rounded-lg p-3 ring-1"
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium">{def.name}</span>
                <VisibilityBadge visibility={def.visibility} />
                {def.enabled ? null : (
                  <span className="bg-muted text-muted-foreground inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] leading-4">
                    <CloudOff aria-hidden className="size-3" />
                    off
                  </span>
                )}
                <span className="ml-auto flex items-center gap-1">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="gap-1 px-2"
                    data-testid="agents-selfchat-btn"
                    onClick={() => navigate(`/chat?a2a=${def.agentId}`)}
                  >
                    <MessageCircle aria-hidden className="size-4" />
                    {t("agents.action.chat")}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="gap-1 px-2"
                    data-testid="agents-edit-btn"
                    onClick={() => onEdit(def)}
                  >
                    <Pencil aria-hidden className="size-4" />
                    {t("agents.action.edit")}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="gap-1 px-2"
                    data-testid="agents-share-btn"
                    onClick={() => toastInfo(t("agents.toast.shareNotReady"))}
                  >
                    <Share2 aria-hidden className="size-4" />
                    {t("agents.action.share")}
                  </Button>
                  <AsyncButton
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="text-destructive gap-1 px-2 hover:text-destructive"
                    action={() => onUnpublish(def)}
                    data-testid="agents-unpublish-btn"
                  >
                    {t("agents.action.unpublish")}
                  </AsyncButton>
                </span>
              </div>
              <p className="text-muted-foreground truncate text-sm">{def.description}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
