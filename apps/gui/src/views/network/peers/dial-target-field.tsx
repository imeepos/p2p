import { useState } from "react";
import { useTranslation } from "react-i18next";

import { EntityCombobox, type PickerOption } from "@/components/picker";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

import type { DialSegments, DialTransport } from "./dial-compose";

interface DialTargetFieldProps {
  segments: DialSegments;
  onSegmentsChange: (next: DialSegments) => void;
  /** 从发现结果带入：选中候选（连带地址三段预填），与手工改写分流 */
  onPickPeer: (peerId: string) => void;
  portInvalid: boolean;
  commandError: string | null;
  pickerOptions: PickerOption[];
}

// 结构化拨号目标（F11）：PeerId/地址/端口三段 + 传输选择；「从发现结果带入」
// 选择器预填三段；端口失焦即时校验就地提示（F14），命令错误同样内联。
export function DialTargetField({
  segments,
  onSegmentsChange,
  onPickPeer,
  portInvalid,
  commandError,
  pickerOptions,
}: DialTargetFieldProps) {
  const { t } = useTranslation();
  const [portTouched, setPortTouched] = useState(false);
  const portErrorId = "dial-port-error";

  const patch = (part: Partial<DialSegments>) => onSegmentsChange({ ...segments, ...part });
  // F14 时机口径：端口校验失焦触发，首次失焦前不打断输入。
  const showPortError = portTouched && portInvalid;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-1">
        <Label htmlFor="dial-peer-pick">{t("picker.dial.bringInLabel")}</Label>
        <EntityCombobox
          id="dial-peer-pick"
          options={pickerOptions}
          value={pickerOptions.some((option) => option.value === segments.peerId) ? segments.peerId : null}
          onChange={(value) => {
            if (value) onPickPeer(value);
          }}
          testId="dial-peer-picker"
        />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="dial-peer-id">{t("picker.dial.peerLabel")}</Label>
        <Input
          id="dial-peer-id"
          value={segments.peerId}
          onChange={(event) => patch({ peerId: event.target.value })}
          className="font-mono text-xs"
          spellCheck={false}
          autoComplete="off"
        />
      </div>
      <div className="grid gap-2 sm:grid-cols-[1fr_7rem_6rem]">
        <div className="flex flex-col gap-1">
          <Label htmlFor="dial-addr">{t("picker.dial.addrLabel")}</Label>
          <Input
            id="dial-addr"
            value={segments.addr}
            onChange={(event) => patch({ addr: event.target.value })}
            placeholder="192.168.1.9"
            className="font-mono text-xs"
            spellCheck={false}
            autoComplete="off"
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="dial-port">{t("picker.dial.portLabel")}</Label>
          <Input
            id="dial-port"
            value={segments.port}
            onChange={(event) => patch({ port: event.target.value })}
            onBlur={() => setPortTouched(true)}
            placeholder="34001"
            inputMode="numeric"
            className="font-mono text-xs"
            autoComplete="off"
            aria-invalid={showPortError}
            aria-describedby={portInvalid ? portErrorId : undefined}
            data-testid="dial-port"
          />
          {showPortError ? (
            <p className="text-destructive text-xs" role="alert" id={portErrorId} data-testid="dial-port-error">
              {t("picker.dial.portInvalid")}
            </p>
          ) : null}
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="dial-transport">{t("picker.dial.transport")}</Label>
          <Select
            value={segments.transport}
            onValueChange={(value) => patch({ transport: value as DialTransport })}
          >
            <SelectTrigger id="dial-transport" data-testid="dial-transport">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="u">{t("picker.dial.transportQuic")}</SelectItem>
              <SelectItem value="t">{t("picker.dial.transportTcp")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      {commandError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="dial-command-error">
          {commandError}
        </p>
      ) : null}
    </div>
  );
}
