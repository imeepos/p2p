// /agents 行内小件（设计 §8.2 第二/三行）：可见性徽章（公开=绿/私有=灰/本机=灰，
// 契约 §17.3-2）、在线启发色点（剩余 TTL >50% 绿 / ≤50% 黄 / 过期或未知灰，§17.3-1）、
// monospace 宿主 peer 短 id + copy、skills 徽章。
import { Copy } from "lucide-react";
import { useTranslation } from "react-i18next";

import { copyText } from "@/views/shared/clipboard";
import { PeerIdShort } from "@/views/shared/peer-id-short";
import type { I18nKey } from "@/i18n/types";
import { cn } from "@/lib/utils";

import type { AgentCardJson, AgentVisibility, DiscoveredAgent } from "@/a2a/types";

export type OnlineLevel = "green" | "yellow" | "gray";

/** 在线启发（契约 §17.3-1）：真实心跳面接入前只渲染色点，无在线/离线文案。 */
export function onlineLevel(nowSecs: number, row: DiscoveredAgent): OnlineLevel {
  const { card, issuedAtSecs } = row;
  if (issuedAtSecs <= 0 || card.ttlSecs <= 0) return "gray";
  const remaining = issuedAtSecs + card.ttlSecs - nowSecs;
  if (remaining <= 0) return "gray";
  return remaining > card.ttlSecs / 2 ? "green" : "yellow";
}

const DOT_CLASS: Record<OnlineLevel, string> = {
  green: "bg-success",
  yellow: "bg-warning",
  gray: "bg-muted-foreground/40",
};

// 动态键显式标注 I18nKey（icon-rail titleKey 同款）：字符串拼接键 TS 落 unknown
const VISIBILITY_KEY: Record<AgentVisibility, I18nKey> = {
  public: "agents.badge.public",
  private: "agents.badge.private",
  local: "agents.badge.local",
};

export function VisibilityBadge({ visibility }: { visibility: AgentVisibility }) {
  const { t } = useTranslation();
  const label = t(VISIBILITY_KEY[visibility]);
  return (
    <span
      data-testid={"agents-visibility-" + visibility}
      className={cn(
        "inline-flex items-center rounded px-1.5 py-0.5 text-[10px] leading-4 font-medium",
        visibility === "public"
          ? "bg-success/15 text-success"
          : "bg-muted text-muted-foreground",
      )}
    >
      {label}
    </span>
  );
}

export function OnlineDot({ level }: { level: OnlineLevel }) {
  const { t } = useTranslation();
  return (
    <span
      aria-label={t("agents.onlineDot")}
      title={t("agents.onlineDot")}
      className={cn("inline-block size-2 shrink-0 rounded-full", DOT_CLASS[level])}
    />
  );
}

/** 第三行：monospace 宿主 peer 短 id + copy + 在线点 + 可见性徽章（level 由行传入）。 */
export function HostPeerCell({ card, level }: { card: AgentCardJson; level: OnlineLevel }) {
  const { t } = useTranslation();
  return (
    <span className="text-muted-foreground inline-flex min-w-0 items-center gap-1 font-mono text-xs">
      <PeerIdShort peerId={card.hostPeer} />
      <button
        type="button"
        aria-label={t("agents.action.copyHost")}
        title={t("agents.action.copyHost")}
        data-testid="agents-copy-host"
        className="hover:text-foreground shrink-0"
        onClick={() => {
          void copyText(card.hostPeer, {
            done: t("agents.toast.copied"),
            failed: t("agents.err.loadFailed"),
          });
        }}
      >
        <Copy aria-hidden className="size-3.5" />
      </button>
      <OnlineDot level={level} />
      <VisibilityBadge visibility={card.visibility} />
    </span>
  );
}
