import { useTranslation } from "react-i18next";
import { useState } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { PickerOption } from "@/components/picker";
import { shortPeerId } from "@/components/picker";
import type { DialReport } from "@/lib/ipc-types";
import { parseDialTarget } from "@/lib/dial-target";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { FORM_VALIDATION_MARK } from "@/views/shared/form-flow";

import {
  composeDialTarget,
  isValidDialPort,
  splitDialAddr,
  splitDialTarget,
  type DialSegments,
} from "./dial-compose";
import { DialDialogFooter } from "./dial-dialog-footer";
import { DialResultPanel } from "./dial-result-panel";
import { DialTargetField } from "./dial-target-field";

interface PeerDialDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 跨卡 URL 契约：#/network/peers?dial=<目标> 打开时预填三段 */
  initialTarget?: string | null;
}

const EMPTY_SEGMENTS: DialSegments = { peerId: "", addr: "", port: "", transport: "u" };

function pickerOptionsOf(peers: Array<{ peerId: string; addrs: string[] }>): PickerOption[] {
  return peers.map((peer) => ({
    value: peer.peerId,
    label: shortPeerId(peer.peerId),
    hint: shortPeerId(peer.peerId),
  }));
}

// 发现结果带入：PeerId 必带；首个可拆地址连带预填主机/端口/传输。
function segmentsFromEntry(peerId: string, addrs: string[]): DialSegments {
  const split = addrs.map(splitDialAddr).find((item) => item !== null);
  if (!split) return { ...EMPTY_SEGMENTS, peerId };
  return { peerId, addr: split.host, port: split.port, transport: split.transport };
}

// 手动拨号（F11 结构化三段 + 发现带入）：提交时三段组装为契约 §6 复合目标，
// 语法裁决仍归 parseDialTarget；端口失焦即时校验（F14）；提交后 Dialog 内
// 展示逐跳结果。
export function PeerDialDialog({ open, onOpenChange, initialTarget }: PeerDialDialogProps) {
  const { t } = useTranslation();
  const dial = useNodeStore((s) => s.dial);
  const running = useNodeStore((s) => s.status?.running ?? false);
  const discovered = useNodeStore(selectPeerList);
  // 三段状态先声明：URL 契约播种（下方渲染期迁移块）与手工编辑共用。
  const [segments, setSegments] = useState<DialSegments>(() =>
    open && initialTarget ? splitDialTarget(initialTarget) : EMPTY_SEGMENTS,
  );
  const [syntaxError, setSyntaxError] = useState<string | null>(null);
  const [report, setReport] = useState<DialReport | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

  // URL 契约预填：挂载即开（open 首渲染即 true）时初值直达（上方惰性初值）；
  // 后续关闭再开走渲染期状态迁移播种（不落 effect）。不可解析目标原串落入
  // PeerId 段，由用户修正（不静默丢弃）。
  const [seededOpen, setSeededOpen] = useState(open);
  if (open !== seededOpen) {
    setSeededOpen(open);
    if (open) {
      setSegments(initialTarget ? splitDialTarget(initialTarget) : EMPTY_SEGMENTS);
      setSyntaxError(null);
      setReport(null);
      setCommandError(null);
    }
  }

  const portInvalid = segments.port.length > 0 && !isValidDialPort(segments.port);
  const canSubmit =
    segments.peerId.trim().length > 0 &&
    segments.addr.trim().length > 0 &&
    segments.port.trim().length > 0 &&
    !portInvalid;

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setSegments(EMPTY_SEGMENTS);
      setSyntaxError(null);
      setReport(null);
      setCommandError(null);
    }
    onOpenChange(next);
  };

  const submit = async (): Promise<DialReport> => {
    // 提交前校验节点运行中：未运行给明确业务提示，不让原始错误冒出来。
    if (!running) {
      setCommandError(t("peers.dial.nodeNotRunning"));
      throw new Error(FORM_VALIDATION_MARK);
    }
    const target = composeDialTarget(segments);
    const parsed = target ? parseDialTarget(target) : null;
    if (!target || !parsed) {
      setSyntaxError(t("peers.dial.invalidFormat"));
      throw new Error(FORM_VALIDATION_MARK);
    }
    setSyntaxError(null);
    const result = await dial(target);
    setReport(result);
    return result;
  };

  const options = pickerOptionsOf(discovered);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("peers.dial.title")}</DialogTitle>
          <DialogDescription>
            {t("peers.dial.targetPlaceholder")}
          </DialogDescription>
        </DialogHeader>
        <DialTargetField
          segments={segments}
          onSegmentsChange={setSegments}
          onPickPeer={(peerId) => {
            const entry = discovered.find((peer) => peer.peerId === peerId);
            setSegments(segmentsFromEntry(peerId, entry?.addrs ?? []));
          }}
          portInvalid={portInvalid}
          commandError={commandError}
          pickerOptions={options}
        />
        {syntaxError ? (
          <p className="text-destructive text-xs" role="alert" data-testid="dial-syntax-error">
            {syntaxError}
          </p>
        ) : null}
        {report && <DialResultPanel report={report} />}
        <DialDialogFooter
          canSubmit={canSubmit}
          onClose={() => handleOpenChange(false)}
          onSubmit={submit}
          onCommandError={setCommandError}
        />
      </DialogContent>
    </Dialog>
  );
}
