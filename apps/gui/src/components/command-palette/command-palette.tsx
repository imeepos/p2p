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

const MAX_PEER_ITEMS = 8;
const MAX_ADDRESS_ITEMS = 8;

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function uniqueAddresses(
  peers: { addrs: string[] }[],
  limit: number,
): string[] {
  const seen = new Set<string>();
  for (const peer of peers) {
    for (const addr of peer.addrs) seen.add(addr);
  }
  return [...seen].slice(0, limit);
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const peers = useNodeStore(selectPeerList);
  useEscapeKey(open, () => onOpenChange(false));

  const close = () => onOpenChange(false);
  const go = (path: string) => {
    close();
    navigate(path);
  };
  const copy = async (text: string, message: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toastSuccess(message);
      close();
    } catch (error) {
      console.error("[palette] 复制失败", error);
      toastError(t("common.copyFailed"), { description: errorText(error) });
    }
  };

  const addresses = uniqueAddresses(peers, MAX_ADDRESS_ITEMS);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
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
            {peers.length > 0 && (
              <>
                <CommandSeparator />
                <CommandGroup heading={t("palette.groups.peers")}>
                  {peers.slice(0, MAX_PEER_ITEMS).map((peer) => (
                    <CommandItem
                      key={peer.peerId}
                      value={"peer " + peer.peerId}
                      keywords={[peer.peerId, ...peer.addrs]}
                      onSelect={() =>
                        void copy(peer.peerId, t("palette.copied"))
                      }
                    >
                      <span className="font-mono text-xs">
                        {peer.peerId.slice(0, 16)}
                      </span>
                      <span className="text-muted-foreground ml-auto text-xs">
                        {t("palette.copyPeerId")}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </>
            )}
            {addresses.length > 0 && (
              <>
                <CommandSeparator />
                <CommandGroup heading={t("palette.groups.addresses")}>
                  {addresses.map((addr) => (
                    <CommandItem
                      key={addr}
                      value={"addr " + addr}
                      onSelect={() =>
                        void copy(addr, t("palette.copied"))
                      }
                    >
                      <span className="font-mono text-xs">{addr}</span>
                      <span className="text-muted-foreground ml-auto text-xs">
                        {t("palette.copyAddress")}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </>
            )}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
