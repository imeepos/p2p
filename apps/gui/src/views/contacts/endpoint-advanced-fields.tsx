import { useTranslation } from "react-i18next";

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

import type { EndpointFormErrors } from "./endpoint-rules";

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
    <p className="text-destructive text-xs" data-testid={testidPrefix + "-" + code}>
      {t(("contacts.endpoint.errors." + code) as I18nKey)}
    </p>
  );
}

interface AdvancedFieldsProps {
  form: AcpEndpoint;
  fieldErrors: EndpointFormErrors | null;
  patch: (field: keyof AcpEndpoint) => (value: string) => void;
  testing: boolean;
  history: string[];
  historyEmptyLabel: string;
}

/** 「高级」折叠区字段（UX3 收敛：wsUrl/token/peer/分享管理全部挂进折叠，
 *  默认以本机 acp-console 值预填；折叠仅视觉收纳，字段保持挂载以留存档草稿） */
export function AdvancedFields({
  form,
  fieldErrors,
  patch,
  testing,
  history,
  historyEmptyLabel,
}: AdvancedFieldsProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-3">
      <p className="text-muted-foreground text-xs">{t("contacts.endpoint.advancedHint")}</p>
      <div className="flex flex-col gap-1">
        <Label htmlFor="contacts-endpoint-wsurl">{t("contacts.endpoint.wsUrlLabel")}</Label>
        <div className="flex gap-2">
          <Input
            id="contacts-endpoint-wsurl"
            className="font-mono text-xs"
            value={form.wsUrl}
            onChange={(e) => patch("wsUrl")(e.target.value)}
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
