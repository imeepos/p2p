import { PencilIcon, PlusIcon, Trash2Icon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ipc } from "@/lib/ipc";
import type { StaticPeerView } from "@/lib/ipc-types";
import { shortPeerId } from "@/lib/peer-name";
import { errorText } from "@/views/shared/form-flow";
import { SettingsGroup } from "./settings-row";
import { StaticPeersEditor } from "./static-peers-editor";

type LoadState = "loading" | "ready" | "failed";
type EditorState = { mode: "add" } | { mode: "edit"; peer: StaticPeerView };

// 静态对端卡（W2b 契约）：static_peers_list/upsert/remove 逐条 CRUD 即落盘，
// 不进设置页主表单草稿；删除二次确认；效果语义 = 节点重启生效（组头注明）。
export function StaticPeersCard() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [peers, setPeers] = useState<StaticPeerView[]>([]);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [editor, setEditor] = useState<EditorState | null>(null);

  const load = useCallback(async () => {
    const { peers: list } = await ipc.staticPeersList();
    setPeers(list);
    setLoadState("ready");
  }, []);

  // 挂载拉取失败可见可重试（services-card 先例的合规 effect 形态）。
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const { peers: list } = await ipc.staticPeersList();
        if (!cancelled) {
          setPeers(list);
          setLoadState("ready");
        }
      } catch (error) {
        console.error("[static-peers] static_peers_list 失败", error);
        if (!cancelled) setLoadState("failed");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleSave = useCallback(
    async (peerId: string, addrs: string[], note: string) => {
      await ipc.staticPeersUpsert(peerId, addrs, note);
      toastSuccess(t("settings.staticPeers.saved"));
      setEditor(null);
      await load();
    },
    [load, t],
  );

  const reportSaveError = useCallback(
    (error: unknown) => {
      console.error("[static-peers] static_peers_upsert 失败", error);
      toastError(t("settings.staticPeers.saveFailed"), {
        description: errorText(error),
      });
    },
    [t],
  );

  const handleRemove = useCallback(
    async (peer: StaticPeerView) => {
      const ok = await confirm({
        title: t("settings.staticPeers.removeConfirmTitle"),
        description: t("settings.staticPeers.removeConfirmDesc", {
          peerId: shortPeerId(peer.peerId),
        }),
        confirmText: t("settings.staticPeers.removeConfirmYes"),
        cancelText: t("common.actions.cancel"),
        destructive: true,
      });
      if (!ok) return;
      try {
        await ipc.staticPeersRemove(peer.peerId);
        toastSuccess(t("settings.staticPeers.removed"));
        await load();
      } catch (error) {
        console.error("[static-peers] static_peers_remove 失败", error);
        toastError(t("settings.staticPeers.removeFailed"), {
          description: errorText(error),
        });
      }
    },
    [confirm, load, t],
  );

  return (
    <SettingsGroup
      title={t("settings.staticPeers.title")}
      description={t("settings.staticPeers.hint")}
    >
      {loadState === "failed" ? (
        <div className="flex flex-col items-start gap-2 py-3" data-testid="static-peers-load-failed">
          <p className="text-destructive text-sm" role="alert">
            {t("settings.staticPeers.loadFailed")}
          </p>
          <AsyncButton
            type="button"
            size="sm"
            variant="outline"
            action={load}
            loadingLabel={t("common.actions.refreshing")}
          >
            {t("common.actions.refresh")}
          </AsyncButton>
        </div>
      ) : null}
      {loadState === "ready" && peers.length === 0 && editor == null ? (
        <p className="text-muted-foreground py-3 text-xs" data-testid="static-peers-empty">
          {t("settings.staticPeers.empty")}
        </p>
      ) : null}
      {loadState === "ready" && peers.length > 0 ? (
        <Table containerClassName="mt-1">
          <TableHeader>
            <TableRow>
              <TableHead>{t("settings.staticPeers.columnPeerId")}</TableHead>
              <TableHead>{t("settings.staticPeers.columnAddrs")}</TableHead>
              <TableHead>{t("settings.staticPeers.columnNote")}</TableHead>
              <TableHead className="text-right">
                {t("settings.staticPeers.columnAction")}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {peers.map((peer) => (
              <TableRow key={peer.peerId} data-testid={`static-peer-row-${peer.peerId}`}>
                <TableCell className="max-w-40 truncate font-mono text-xs" title={peer.peerId}>
                  {shortPeerId(peer.peerId)}
                </TableCell>
                <TableCell className="max-w-56 truncate font-mono text-xs" title={peer.addrs.join(", ")}>
                  {peer.addrs.join(", ")}
                </TableCell>
                <TableCell className="max-w-40 truncate text-xs">
                  {peer.note}
                </TableCell>
                <TableCell className="text-right">
                  <div className="flex justify-end gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label={t("settings.staticPeers.edit")}
                      data-testid={`static-peer-edit-${peer.peerId}`}
                      onClick={() => setEditor({ mode: "edit", peer })}
                    >
                      <PencilIcon aria-hidden />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label={t("settings.staticPeers.remove")}
                      data-testid={`static-peer-remove-${peer.peerId}`}
                      onClick={() => void handleRemove(peer)}
                    >
                      <Trash2Icon aria-hidden />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}
      {editor != null && loadState === "ready" ? (
        <div className="py-2">
          <StaticPeersEditor
            initial={editor.mode === "edit" ? editor.peer : null}
            onCancel={() => setEditor(null)}
            onSubmit={handleSave}
            onSaveError={reportSaveError}
          />
        </div>
      ) : null}
      {loadState === "ready" && editor == null ? (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="w-fit"
          data-testid="static-peers-add"
          onClick={() => setEditor({ mode: "add" })}
        >
          <PlusIcon aria-hidden />
          {t("settings.staticPeers.add")}
        </Button>
      ) : null}
    </SettingsGroup>
  );
}
