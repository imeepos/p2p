import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { EntityCombobox } from "@/components/picker";
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
import { Switch } from "@/components/ui/switch";
import { useFriendPickerOptions } from "@/views/shared/peer-options";

import type { UseRdPageModel } from "./use-rd-model";
import { useRdCardSettings } from "./use-rd-model";
import { RdFrameCanvas } from "./rd-frame-canvas";

interface Props {
  model: UseRdPageModel;
}

// 远程桌面卡（gui-contract §21）：被控端开关/审批队列 + 控制端连接 + 质量。
// 开关/帧率初值读设置页持久默认（useRdCardSettings），本卡仍可临时改并
// 随 startHost/rdQualitySet 下发（运行态覆盖能力保留）。
export function RemoteDesktopCard({ model }: Props) {
  const { t } = useTranslation();
  const settings = useRdCardSettings();
  const [peer, setPeer] = useState("");
  // R2-05：viewer 目标 = 好友选择器 + 手输兜底（与通用隧道卡同构）
  const friendOptions = useFriendPickerOptions();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("remoteAccess.rd.title")}</CardTitle>
        <CardDescription>{t("remoteAccess.rd.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* 被控端 host */}
        <section className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">
                {t("remoteAccess.rd.host.title")}
              </div>
              <div className="text-xs text-muted-foreground">
                {t("remoteAccess.rd.host.description")}
              </div>
            </div>
            {model.host.running ? (
              <Badge>
                {t("remoteAccess.rd.host.statusEnabled")}
              </Badge>
            ) : (
              <Badge variant="outline">
                {t("remoteAccess.rd.host.statusDisabled")}
              </Badge>
            )}
          </div>
          <div className="flex items-center justify-between gap-3">
            <Label htmlFor="rd-approval">
              {t("remoteAccess.rd.host.approvalLabel")}
            </Label>
            <Switch
              id="rd-approval"
              checked={settings.approvalOn}
              onCheckedChange={settings.setApproval}
            />
          </div>
          <div className="flex gap-2">
            <Button
              className="flex-1"
              disabled={model.busy === "start" || model.host.running}
              onClick={() => void model.startHost(settings.approvalOn)}
            >
              {model.busy === "start"
                ? t("remoteAccess.rd.host.starting")
                : t("remoteAccess.rd.host.start")}
            </Button>
            <Button
              className="flex-1"
              variant="outline"
              disabled={model.busy === "stop" || !model.host.running}
              onClick={() => void model.stopHost()}
            >
              {model.busy === "stop"
                ? t("remoteAccess.rd.host.stopping")
                : t("remoteAccess.rd.host.stop")}
            </Button>
          </div>
          <div className="flex gap-4 text-xs text-muted-foreground">
            <span>
              {t("remoteAccess.rd.host.sessions")}: {model.host.sessionCount}
            </span>
            <span>
              {t("remoteAccess.rd.host.fps")}: {model.host.fps}
            </span>
          </div>
        </section>

        {/* 待审批队列 */}
        <section className="space-y-2">
          <div className="text-sm font-medium">
            {t("remoteAccess.rd.approvals.title")}
          </div>
          {model.host.pendingApprovals.length === 0 ? (
            <div className="text-xs text-muted-foreground">
              {t("remoteAccess.rd.approvals.empty")}
            </div>
          ) : (
            <ul className="space-y-2">
              {model.host.pendingApprovals.map((peerId) => (
                <li
                  key={peerId}
                  className="flex items-center justify-between gap-2 rounded border px-3 py-2"
                >
                  <span className="truncate font-mono text-xs">{peerId}</span>
                  <span className="flex shrink-0 gap-2">
                    <Button
                      size="sm"
                      onClick={() => void model.approve(peerId)}
                    >
                      {t("remoteAccess.rd.approvals.approve")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void model.deny(peerId)}
                    >
                      {t("remoteAccess.rd.approvals.deny")}
                    </Button>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* 质量 */}
        <section className="space-y-2">
          <Label htmlFor="rd-fps">{t("remoteAccess.rd.quality.fps")}</Label>
          <div className="flex items-center gap-2">
            <Input
              id="rd-fps"
              type="number"
              min={1}
              max={60}
              value={settings.fps}
              onChange={(event) => settings.setFps(Number(event.target.value))}
              className="w-24"
            />
            <Button
              variant="outline"
              disabled={!model.host.running}
              onClick={() => void model.setQuality(settings.fps)}
            >
              {t("remoteAccess.rd.quality.title")}
            </Button>
          </div>
        </section>

        {/* 控制端 viewer */}
        <section className="space-y-2">
          <div className="text-sm font-medium">
            {t("remoteAccess.rd.viewer.title")}
          </div>
          <div className="text-xs text-muted-foreground">
            {t("remoteAccess.rd.viewer.description")}
          </div>
          {model.viewer.connected ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <Badge>
                  {t("remoteAccess.rd.viewer.connected")}
                </Badge>
                <Button variant="outline" onClick={() => void model.closeViewer()}>
                  {t("remoteAccess.rd.viewer.disconnect")}
                </Button>
              </div>
              <RdFrameCanvas model={model} />
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              <EntityCombobox
                id="rd-viewer-peer-pick"
                testId="rd-viewer-peer-pick"
                options={friendOptions}
                value={
                  friendOptions.some((option) => option.value === peer)
                    ? peer
                    : null
                }
                onChange={(next) => setPeer(next ?? "")}
              />
              <div className="flex gap-2">
                <Input
                  placeholder={t("remoteAccess.rd.viewer.peerPlaceholder")}
                  value={peer}
                  onChange={(event) => setPeer(event.target.value)}
                />
                <Button
                  disabled={model.busy === "connect" || peer.trim() === ""}
                  onClick={() => void model.connectViewer(peer.trim())}
                >
                  {model.busy === "connect"
                    ? t("remoteAccess.rd.viewer.connecting")
                    : t("remoteAccess.rd.viewer.connect")}
                </Button>
              </div>
            </div>
          )}
          {model.lastError ? (
            <div className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {model.lastError}
            </div>
          ) : null}
        </section>
      </CardContent>
    </Card>
  );
}