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
import type { AcpWorkspace } from "@/acp/share-admin-client";
import type { ShareScope, ShareTtlKey } from "@/acp/share-model";
import type { I18nKey } from "@/i18n/types";

const SHARE_TTL_KEYS: ShareTtlKey[] = ["1h", "24h", "7d"];

const TTL_LABEL_KEY: Record<ShareTtlKey, I18nKey> = {
  "1h": "acp.share.ttl1h",
  "24h": "acp.share.ttl24h",
  "7d": "acp.share.ttl7d",
};

export interface ShareCreateFieldsProps {
  scope: ShareScope;
  onScopeChange: (scope: ShareScope) => void;
  ttl: ShareTtlKey;
  onTtlChange: (ttl: ShareTtlKey) => void;
  activations: string;
  onActivationsChange: (value: string) => void;
  note: string;
  onNoteChange: (value: string) => void;
  errors: { activations?: boolean; note?: boolean } | null;
  onClearErrors: () => void;
  workspaces: AcpWorkspace[] | null;
  workspaceId: string;
  onWorkspaceChange: (id: string) => void;
}

/** 创建表单字段网格（scope/工作区/有效期/激活次数/备注）：弹层与本地卡共用。 */
export function ShareCreateFields(props: ShareCreateFieldsProps) {
  const { t } = useTranslation();
  const errors = props.errors;
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      <div className="flex flex-col gap-1">
        <Label>{t("acp.share.scopeLabel")}</Label>
        <Select value={props.scope} onValueChange={(v) => props.onScopeChange(v as ShareScope)}>
          <SelectTrigger className="w-full" data-testid="acp-share-scope">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="sandbox">{t("acp.share.scopeSandbox")}</SelectItem>
            <SelectItem value="workspace">{t("acp.share.scopeWorkspace")}</SelectItem>
          </SelectContent>
        </Select>
        {props.scope === "workspace" ? (
          props.workspaces && props.workspaces.length > 0 ? (
            <Select
              value={props.workspaceId || props.workspaces[0].id}
              onValueChange={props.onWorkspaceChange}
            >
              <SelectTrigger className="w-full" data-testid="acp-share-workspace">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {props.workspaces.map((ws) => (
                  <SelectItem key={ws.id} value={ws.id}>
                    {ws.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <p className="text-muted-foreground text-xs">{t("acp.share.scopeWorkspaceHint")}</p>
          )
        ) : null}
      </div>
      <div className="flex flex-col gap-1">
        <Label>{t("acp.share.ttlLabel")}</Label>
        <Select value={props.ttl} onValueChange={(v) => props.onTtlChange(v as ShareTtlKey)}>
          <SelectTrigger className="w-full" data-testid="acp-share-ttl">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SHARE_TTL_KEYS.map((key) => (
              <SelectItem key={key} value={key}>
                {t(TTL_LABEL_KEY[key])}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="acp-share-activations">{t("acp.share.activationsLabel")}</Label>
        <Input
          id="acp-share-activations"
          type="number"
          min={1}
          value={props.activations}
          onChange={(e) => {
            props.onActivationsChange(e.target.value);
            props.onClearErrors();
          }}
          data-testid="acp-share-activations"
        />
        {errors?.activations ? (
          <p className="text-destructive text-xs" data-testid="acp-share-error-activations">
            {t("acp.share.validation.activations")}
          </p>
        ) : null}
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="acp-share-note">{t("acp.share.noteLabel")}</Label>
        <Input
          id="acp-share-note"
          value={props.note}
          onChange={(e) => {
            props.onNoteChange(e.target.value);
            props.onClearErrors();
          }}
          placeholder={t("acp.share.notePlaceholder")}
          autoComplete="off"
          data-testid="acp-share-note"
        />
        {errors?.note ? (
          <p className="text-destructive text-xs" data-testid="acp-share-error-note">
            {t("acp.share.validation.noteTooLong")}
          </p>
        ) : null}
      </div>
    </div>
  );
}