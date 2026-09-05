import { useTranslation } from "react-i18next";

import { CopyButton } from "@/components/monitor/copy-button";
import { HopTimeline } from "@/components/monitor/hop-timeline";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/monitor/sheet";
import { Separator } from "@/components/ui/separator";
import type { PeerEntry } from "@/stores/node-store";
import { PEER_ID_PREFIX_LEN } from "@/views/shared/peer-id-cell";

function SectionTitle({ children }: { children: string }) {
  return <h3 className="text-sm font-medium">{children}</h3>;
}

function PeerIdentitySection({ peer }: { peer: PeerEntry }) {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col gap-2">
      <SectionTitle>{t("peers.detail.fullPeerId")}</SectionTitle>
      <div className="flex items-start gap-1">
        <p className="min-w-0 flex-1 font-mono text-xs break-all">
          {peer.peerId}
        </p>
        <CopyButton value={peer.peerId} className="size-6 shrink-0" />
      </div>
    </section>
  );
}

function PeerAddrSection({ addrs }: { addrs: string[] }) {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col gap-2">
      <SectionTitle>{t("peers.detail.addrs")}</SectionTitle>
      {addrs.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("common.labels.none")}
        </p>
      ) : (
        <ul className="flex flex-col gap-1">
          {addrs.map((addr) => (
            <li key={addr} className="font-mono text-xs break-all">
              {addr}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function PeerHopSection({ hops }: { hops: PeerEntry["hops"] }) {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col gap-3">
      <SectionTitle>{t("peers.detail.hopHistory")}</SectionTitle>
      {hops.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("peers.detail.emptyHops")}
        </p>
      ) : (
        <HopTimeline hops={hops} />
      )}
    </section>
  );
}

interface PeerDetailSheetProps {
  peer: PeerEntry | null;
  onOpenChange: (open: boolean) => void;
}

// 节点详情抽屉：完整 PeerId、地址列表、dial_hop 逐跳历史时间线。
export function PeerDetailSheet({ peer, onOpenChange }: PeerDetailSheetProps) {
  const { t } = useTranslation();

  return (
    <Sheet open={peer !== null} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>{t("peers.detail.title")}</SheetTitle>
          <SheetDescription className="font-mono">
            {peer ? `${peer.peerId.slice(0, PEER_ID_PREFIX_LEN)}…` : ""}
          </SheetDescription>
        </SheetHeader>
        {peer && (
          <div className="flex flex-col gap-5">
            <PeerIdentitySection peer={peer} />
            <Separator />
            <PeerAddrSection addrs={peer.addrs} />
            <Separator />
            <PeerHopSection hops={peer.hops} />
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
