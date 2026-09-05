import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { MENU_ENTRIES } from "@/config/menu.def";
import { useEscapeKey } from "@/hooks/use-hotkeys";
import { useNodeStore, selectPeerList } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";
import { subscribeOpenCommandPalette } from "./palette-bus";

const MAX_PEER_ITEMS = 8;
const MAX_ADDRESS_ITEMS = 8;

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface PeerRow {
  peerId: string;
  addrs: string[];
}

type CopyFn = (text: string, message: string) => Promise<void>;

function uniqueAddresses(peers: PeerRow[], limit: number): string[] {
  const seen = new Set<string>();
  for (const peer of peers) {
    for (const addr of peer.addrs) seen.add(addr);
  }
  return [...seen].slice(0, limit);
}

function useClipboardCopy(onClose: () => void): CopyFn {
  const { t } = useTranslation();
  return useCallback(
    async (text, message) => {
      try {
        await navigator.clipboard.writeText(text);
        toastSuccess(message);
        onClose();
      } catch (error) {
        console.error("[palette] 复制失败", error);
        toastError(t("common.copyFailed"), { description: errorText(error) });
      }
    },
    [onClose, t],
  );
}

function MenuGroup({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const go = (path: string) => {
    onClose();
    navigate(path);
  };
  return (
    <CommandGroup heading={t("palette.groups.menu")}>
      {MENU_ENTRIES.map((entry) => (
        <CommandItem
          key={entry.path}
          value={"menu " + entry.path + " " + t(entry.titleKey)}
          onSelect={() => go(entry.path)}
        >
          <entry.icon className="size-4" />
          <span>{t(entry.titleKey)}</span>
        </CommandItem>
      ))}
    </CommandGroup>
  );
}

function PeerGroup({ peers, onCopy }: { peers: PeerRow[]; onCopy: CopyFn }) {
  const { t } = useTranslation();
  return (
    <CommandGroup heading={t("palette.groups.peers")}>
      {peers.map((peer) => (
        <CommandItem
          key={peer.peerId}
          value={"peer " + peer.peerId}
          keywords={[peer.peerId, ...peer.addrs]}
          onSelect={() => void onCopy(peer.peerId, t("palette.copied"))}
        >
          <span className="font-mono text-xs">{peer.peerId.slice(0, 16)}</span>
          <span className="text-muted-foreground ml-auto text-xs">
            {t("palette.copyPeerId")}
          </span>
        </CommandItem>
      ))}
    </CommandGroup>
  );
}

function AddressGroup({ addresses, onCopy }: { addresses: string[]; onCopy: CopyFn }) {
  const { t } = useTranslation();
  return (
    <CommandGroup heading={t("palette.groups.addresses")}>
      {addresses.map((addr) => (
        <CommandItem
          key={addr}
          value={"addr " + addr}
          onSelect={() => void onCopy(addr, t("palette.copied"))}
        >
          <span className="font-mono text-xs">{addr}</span>
          <span className="text-muted-foreground ml-auto text-xs">
            {t("palette.copyAddress")}
          </span>
        </CommandItem>
      ))}
    </CommandGroup>
  );
}

function PaletteItems({ onClose }: { onClose: () => void }) {
  const peers = useNodeStore(selectPeerList);
  const addresses = uniqueAddresses(peers, MAX_ADDRESS_ITEMS);
  const copy = useClipboardCopy(onClose);
  return (
    <>
      <MenuGroup onClose={onClose} />
      {peers.length > 0 && (
        <>
          <CommandSeparator />
          <PeerGroup peers={peers.slice(0, MAX_PEER_ITEMS)} onCopy={copy} />
        </>
      )}
      {addresses.length > 0 && (
        <>
          <CommandSeparator />
          <AddressGroup addresses={addresses} onCopy={copy} />
        </>
      )}
    </>
  );
}

// 底部快捷键说明与实现保持一致：Cmd/Ctrl+K 打开、1..9 切前九个页面、Esc 关闭
function PaletteFooter() {
  const { t } = useTranslation();
  return (
    <div className="text-muted-foreground flex flex-wrap items-center gap-x-4 gap-y-1 border-t px-3 py-2 text-xs">
      <span>{t("palette.hints.open")}</span>
      <span>{t("palette.hints.navigate")}</span>
      <span>{t("palette.hints.close")}</span>
    </div>
  );
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const { t } = useTranslation();
  const [requestedOpen, setRequestedOpen] = useState(false);
  // 顶栏等壳层组件经事件总线请求打开；受控 props 仍是权威状态源
  useEffect(() => subscribeOpenCommandPalette(() => setRequestedOpen(true)), []);
  const isOpen = open || requestedOpen;

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) setRequestedOpen(false);
      onOpenChange(next);
    },
    [onOpenChange],
  );
  // 面板内部条目的关闭动作必须显式传 false：无参调用会把 undefined
  // 写回受控状态，绕过受控契约
  const close = useCallback(() => handleOpenChange(false), [handleOpenChange]);
  useEscapeKey(isOpen, close);

  return (
    <Dialog open={isOpen} onOpenChange={handleOpenChange}>
      <DialogContent
        className="overflow-hidden p-0 sm:max-w-lg"
        showCloseButton={false}
        aria-describedby={undefined}
      >
        <DialogTitle className="sr-only">{t("palette.open")}</DialogTitle>
        <Command>
          <CommandInput placeholder={t("palette.placeholder")} autoFocus />
          <CommandList>
            <CommandEmpty>{t("palette.empty")}</CommandEmpty>
            <PaletteItems onClose={close} />
          </CommandList>
          <PaletteFooter />
        </Command>
      </DialogContent>
    </Dialog>
  );
}
