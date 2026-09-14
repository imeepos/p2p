import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { EntityCombobox, type PickerOption } from "@/components/picker";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ipc } from "@/lib/ipc";
import type { TunnelOpenReport } from "@/lib/ipc-types";
import { shortPeerId, usePeerNameSource } from "@/lib/peer-name";

import { matchErrorCode } from "./tunnel-flow";
import type { TunnelBannerInput } from "./tunnel-terminal-banner";
import { TargetPortField } from "./target-port-field";
import { TunnelErrorBox } from "./tunnel-error-box";

interface Props {
  onOpened: (report: TunnelOpenReport) => void;
  onError: (message: string) => void;
  onBanner: (banner: TunnelBannerInput) => void;
}

// 好友选择器选项：label = 昵称（备注回退），hint = 缩略 PeerId。
function friendOptions(
  friends: { peerId: string; nickname: string; note?: string | null }[],
): PickerOption[] {
  return friends.map((f) => ({
    value: f.peerId,
    label: f.nickname || f.note || shortPeerId(f.peerId),
    hint: shortPeerId(f.peerId),
  }));
}

// 好友选择器 + 手填兜底（gap-matrix §4.2）：选择即回填下方输入框，
// 输入框始终是唯一真值源，手填路径原样保留。
function PeerPickerField({
  peer,
  onPick,
}: {
  peer: string;
  onPick: (peerId: string) => void;
}) {
  const { t } = useTranslation();
  const friends = usePeerNameSource();
  const options = friendOptions(friends);
  return (
    <div className="space-y-2">
      <Label htmlFor="tunnel-peer-picker">
        {t("remoteAccess.tunnel.friendPick")}
      </Label>
      <EntityCombobox
        id="tunnel-peer-picker"
        testId="tunnel-peer-picker"
        options={options}
        value={options.some((o) => o.value === peer) ? peer : null}
        onChange={(next) => onPick(next ?? "")}
        placeholder={t("remoteAccess.tunnel.friendPickPlaceholder")}
        searchPlaceholder={t("remoteAccess.tunnel.friendPickSearch")}
        emptyText={t("remoteAccess.tunnel.friendPickEmpty")}
      />
    </div>
  );
}

// 通用服务开隧道卡片（gui-contract §19.3-9）：端口 + 被访节点 PeerId →
// tunnel_open（target 固定拼为 127.0.0.1:<port> 字面量，服务端校验）。
// 终态上抛页面横幅：成功含链接与 peer，失败含原因与闭集错误码。
export function GenericTunnelCard({ onOpened, onError, onBanner }: Props) {
  const { t } = useTranslation();
  const [port, setPort] = useState("");
  const [peer, setPeer] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const open = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const report = await ipc.tunnelOpen(`127.0.0.1:${port.trim()}`, peer.trim());
      onOpened(report);
      onBanner({ tone: "success", link: report.openUrl, peer: peer.trim() });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 通用开启失败", error);
      setLastError(message);
      onError(message);
      onBanner({ tone: "error", reason: message, code: matchErrorCode(message) });
    } finally {
      setBusy(false);
    }
  }, [onBanner, onError, onOpened, peer, port]);

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("remoteAccess.generic.title")}</CardTitle>
        <CardDescription>{t("remoteAccess.generic.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <TargetPortField
          inputId="tunnel-generic-target"
          label={t("remoteAccess.generic.port")}
          placeholder={t("remoteAccess.generic.portPlaceholder")}
          hint={t("remoteAccess.generic.portHint")}
          value={port}
          onChange={setPort}
        />
        <PeerPickerField peer={peer} onPick={setPeer} />
        <div className="space-y-2">
          <Label htmlFor="tunnel-generic-peer">
            {t("remoteAccess.generic.peer")}
          </Label>
          <Input
            id="tunnel-generic-peer"
            value={peer}
            placeholder={t("remoteAccess.generic.peerPlaceholder")}
            onChange={(e) => setPeer(e.target.value)}
          />
          <p className="text-muted-foreground text-xs">
            {t("remoteAccess.tunnel.peerManualHint")}
          </p>
        </div>
        <Button
          type="button"
          onClick={() => void open()}
          disabled={busy || port.trim() === "" || peer.trim() === ""}
        >
          {busy
            ? t("remoteAccess.generic.opening")
            : t("remoteAccess.generic.open")}
        </Button>
        {lastError && <TunnelErrorBox message={lastError} showNodeHint />}
      </CardContent>
    </Card>
  );
}
