import { useTranslation } from "react-i18next";

import { EntityCombobox, shortPeerId, type PickerOption } from "@/components/picker";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { AcpEndpoint } from "@/acp/protocol";
import type { I18nKey } from "@/i18n/types";

import type { EndpointFormErrors, EndpointTargetOption } from "./endpoint-rules";

export function EndpointFieldError({
  code,
  testidPrefix = "contacts-endpoint-error",
}: {
  code?: string;
  testidPrefix?: string;
}) {
  const { t } = useTranslation();
  if (!code) return null;
  return (
    <p className="text-destructive text-xs" role="alert" data-testid={testidPrefix + "-" + code}>
      {t(("contacts.endpoint.errors." + code) as I18nKey)}
    </p>
  );
}

interface WsUrlFieldProps {
  form: AcpEndpoint;
  fieldErrors: EndpointFormErrors | null;
  patch: (field: keyof AcpEndpoint) => (value: string) => void;
  testing: boolean;
  history: string[];
  historyEmptyLabel: string;
}

/** WS 地址主字段（F25）：默认展开可见；wsUrl 历史值下拉并行保留 */
export function WsUrlField({
  form,
  fieldErrors,
  patch,
  testing,
  history,
  historyEmptyLabel,
}: WsUrlFieldProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor="contacts-endpoint-wsurl">{t("contacts.endpoint.wsUrlLabel")}</Label>
      <div className="flex gap-2">
        <Input
          id="contacts-endpoint-wsurl"
          className="font-mono text-xs"
          value={form.wsUrl}
          onChange={(e) => patch("wsUrl")(e.target.value)}
          placeholder={t("contacts.endpoint.wsUrlPlaceholder")}
          autoComplete="off"
          data-testid="contacts-endpoint-wsurl"
        />
        {history.length > 0 ? (
          <Select value="" onValueChange={(v) => v && patch("wsUrl")(v)} disabled={testing}>
            <SelectTrigger className="w-28" data-testid="contacts-endpoint-history">
              <SelectValue placeholder={historyEmptyLabel} />
            </SelectTrigger>
            <SelectContent>
              {history.map((url) => (
                <SelectItem key={url} value={url} data-testid={"contacts-endpoint-history-" + url}>
                  {url}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : null}
      </div>
      <EndpointFieldError code={fieldErrors?.wsUrl} />
    </div>
  );
}

interface TargetPickerProps {
  form: AcpEndpoint;
  candidates: EndpointTargetOption[];
  fieldErrors: EndpointFormErrors | null;
  patch: (field: keyof AcpEndpoint) => (value: string) => void;
}

/** 目标节点选择（F25 降级为辅助填充）：统一关联选择器，选中即回填 peer；
 *  peer 自由文本输入仍保留在「高级」折叠内作兜底。 */
export function EndpointTargetPicker({ form, candidates, fieldErrors, patch }: TargetPickerProps) {
  const { t } = useTranslation();
  const options: PickerOption[] = candidates.map((option) => ({
    value: option.peer,
    label: option.label,
    hint: shortPeerId(option.peer),
  }));
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor="contacts-endpoint-target">{t("picker.endpointAuxLabel")}</Label>
      <EntityCombobox
        id="contacts-endpoint-target"
        options={options}
        value={options.some((option) => option.value === form.peer) ? form.peer : null}
        onChange={(value) => {
          if (value) patch("peer")(value);
          else patch("peer")("");
        }}
        testId="contacts-endpoint-target"
      />
      <EndpointFieldError code={fieldErrors?.peer} testidPrefix="contacts-endpoint-target-error" />
    </div>
  );
}

interface AdvancedFieldsProps {
  form: AcpEndpoint;
  fieldErrors: EndpointFormErrors | null;
  patch: (field: keyof AcpEndpoint) => (value: string) => void;
}

/** 「高级」折叠区字段：token/peer 兜底/分享管理；折叠仅视觉收纳，
 *  字段保持挂载以留存档草稿（wsUrl 已上移为主字段，不再驻留折叠）。 */
export function AdvancedFields({ form, fieldErrors, patch }: AdvancedFieldsProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-3">
      <p className="text-muted-foreground text-xs">{t("contacts.endpoint.advancedHint")}</p>
      <div className="flex flex-col gap-1">
        <Label htmlFor="contacts-endpoint-token">{t("contacts.endpoint.tokenLabel")}</Label>
        <Input
          id="contacts-endpoint-token"
          type="password"
          value={form.token}
          onChange={(e) => patch("token")(e.target.value)}
          placeholder={t("contacts.endpoint.tokenPlaceholder")}
          autoComplete="off"
          data-testid="contacts-endpoint-token"
        />
        <EndpointFieldError code={fieldErrors?.token} />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="contacts-endpoint-peer">{t("contacts.endpoint.peerLabel")}</Label>
        <Input
          id="contacts-endpoint-peer"
          className="font-mono text-xs"
          value={form.peer}
          onChange={(e) => patch("peer")(e.target.value)}
          placeholder={t("contacts.endpoint.peerPlaceholder")}
          autoComplete="off"
          data-testid="contacts-endpoint-peer"
        />
        <EndpointFieldError code={fieldErrors?.peer} />
      </div>
      <div className="flex flex-col gap-2 border-t pt-3">
        <Label className="text-muted-foreground">{t("contacts.endpoint.adminSectionLabel")}</Label>
        <p className="text-muted-foreground text-xs">{t("contacts.endpoint.adminHint")}</p>
        <div className="grid gap-2 sm:grid-cols-2">
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-adminurl">{t("contacts.endpoint.adminUrlLabel")}</Label>
            <Input
              id="contacts-endpoint-adminurl"
              className="font-mono text-xs"
              value={form.adminUrl ?? ""}
              onChange={(e) => patch("adminUrl")(e.target.value)}
              placeholder={t("contacts.endpoint.adminUrlPlaceholder")}
              autoComplete="off"
              data-testid="contacts-endpoint-adminurl"
            />
            <EndpointFieldError code={fieldErrors?.adminUrl} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="contacts-endpoint-admintoken">{t("contacts.endpoint.adminTokenLabel")}</Label>
            <Input
              id="contacts-endpoint-admintoken"
              type="password"
              value={form.adminToken ?? ""}
              onChange={(e) => patch("adminToken")(e.target.value)}
              placeholder={t("contacts.endpoint.adminTokenPlaceholder")}
              autoComplete="off"
              data-testid="contacts-endpoint-admintoken"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
