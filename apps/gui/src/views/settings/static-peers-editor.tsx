import { zodResolver } from "@hookform/resolvers/zod";
import { FormProvider, useForm } from "react-hook-form";
import { z } from "zod";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { isValidPeerId } from "@/lib/dial-target";
import { ErrorText } from "@/views/shared/error-text";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";
import {
  addrRowsField,
  fromRows,
  toRows,
} from "@/views/shared/address-rules";
import type { StaticPeerView } from "@/lib/ipc-types";

// 草稿校验：peerId base58（与拨号面同规则族）；地址复用 §6 行规则并要求
// 至少一行；note 自由文本。错误码经 common.validation 双语渲染。
const staticPeerSchema = z.object({
  peerId: z
    .string()
    .min(1, "peerIdRequired")
    .refine(isValidPeerId, "peerIdFormat"),
  addrs: addrRowsField("addrDuplicate").refine(
    (rows) => rows.length >= 1,
    "addrRequired",
  ),
  note: z.string(),
});

type StaticPeerDraft = z.infer<typeof staticPeerSchema>;

interface StaticPeersEditorProps {
  // 非 null = 编辑既有条目：PeerId 锁定（upsert 以其为键，改键=另立条目）。
  initial: StaticPeerView | null;
  onCancel: () => void;
  onSubmit: (peerId: string, addrs: string[], note: string) => Promise<void>;
  onSaveError: (error: unknown) => void;
}

// 静态对端编辑器：独立 RHF 表单（不进设置页主草稿——upsert 即落盘），
// 地址行编辑复用 AddressListEditor；提交经 AsyncButton 走加载态与错误回调。
export function StaticPeersEditor({
  initial,
  onCancel,
  onSubmit,
  onSaveError,
}: StaticPeersEditorProps) {
  const { t } = useTranslation();
  const form = useForm<StaticPeerDraft>({
    resolver: zodResolver(staticPeerSchema),
    defaultValues: initial
      ? {
          peerId: initial.peerId,
          addrs: toRows(initial.addrs),
          note: initial.note,
        }
      : { peerId: "", addrs: [], note: "" },
  });
  const peerIdError = form.formState.errors.peerId?.message;
  const addrsError = form.formState.errors.addrs as
    | { root?: { message?: string } }
    | undefined;
  const submit = form.handleSubmit(async (values) => {
    await onSubmit(values.peerId.trim(), fromRows(values.addrs), values.note.trim());
  });

  return (
    <FormProvider {...form}>
      <div
        className="flex flex-col gap-3 rounded-md border p-3"
        data-testid="static-peers-editor"
      >
      <p className="text-sm font-medium">
        {initial != null
          ? t("settings.staticPeers.editorEditTitle")
          : t("settings.staticPeers.editorAddTitle")}
      </p>
      <div className="flex flex-col gap-1">
        <Label htmlFor="static-peer-id">
          {t("settings.staticPeers.peerIdLabel")}
        </Label>
        <Input
          id="static-peer-id"
          className="font-mono text-xs"
          disabled={initial != null}
          aria-invalid={peerIdError != null ? true : undefined}
          placeholder="2and9…8Jkh (43-45)"
          {...form.register("peerId", {
            onBlur: () => void form.trigger("peerId"),
          })}
        />
        <p className="text-muted-foreground text-xs">
          {initial != null
            ? t("settings.staticPeers.peerIdLockedHint")
            : t("settings.staticPeers.peerIdHint")}
        </p>
        <ErrorText code={peerIdError} />
      </div>
      <AddressListEditor
        control={form.control}
        name="addrs"
        label={t("settings.staticPeers.addrsLabel")}
        hint={t("settings.staticPeers.addrsGuide")}
        placeholder="192.168.1.10/u4222"
      />
      <ErrorText code={addrsError?.root?.message} />
      <div className="flex flex-col gap-1">
        <Label htmlFor="static-peer-note">
          {t("settings.staticPeers.noteLabel")}
        </Label>
        <Input
          id="static-peer-note"
          className="font-mono text-xs"
          placeholder={t("settings.staticPeers.notePlaceholder")}
          {...form.register("note")}
        />
      </div>
      <div className="flex items-center gap-2">
        <AsyncButton
          type="button"
          size="sm"
          action={submit}
          loadingLabel={t("common.actions.saving")}
          onError={onSaveError}
        >
          {t("settings.staticPeers.save")}
        </AsyncButton>
        <Button type="button" size="sm" variant="ghost" onClick={onCancel}>
          {t("common.actions.cancel")}
        </Button>
      </div>
      </div>
    </FormProvider>
  );
}
