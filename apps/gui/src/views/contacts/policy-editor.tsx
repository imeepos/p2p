import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  POLICY_KIND_BUCKETS,
  type EndpointPolicy,
  type PolicyKindBucket,
  type PolicyTier,
} from "@/acp/endpoint-policy";

const TIER_KEYS: Record<PolicyTier, "contacts.policy.tier.ask" | "contacts.policy.tier.allow" | "contacts.policy.tier.deny"> = {
  ask: "contacts.policy.tier.ask",
  allow: "contacts.policy.tier.allow",
  deny: "contacts.policy.tier.deny",
};

const BUCKET_KEYS: Record<PolicyKindBucket, "contacts.policy.toolKind.read" | "contacts.policy.toolKind.write" | "contacts.policy.toolKind.execute" | "contacts.policy.toolKind.fetch" | "contacts.policy.toolKind.think" | "contacts.policy.toolKind.other"> = {
  read: "contacts.policy.toolKind.read",
  write: "contacts.policy.toolKind.write",
  execute: "contacts.policy.toolKind.execute",
  fetch: "contacts.policy.toolKind.fetch",
  think: "contacts.policy.toolKind.think",
  other: "contacts.policy.toolKind.other",
};

interface PolicyEditorProps {
  policy: EndpointPolicy;
  onChange: (policy: EndpointPolicy) => void;
}

// 权限策略配置（§3.3 第 3 块）：六类型桶默认档 + 标题精确例外；变更即写
// 入 endpoint-meta，对后续到达的权限请求即时生效（自动应答红线见
// endpoint-policy.ts：allow 只逐次放行）。
export function PolicyEditor({ policy, onChange }: PolicyEditorProps) {
  const { t } = useTranslation();

  const setDefault = (bucket: PolicyKindBucket, tier: PolicyTier) => {
    onChange({ ...policy, defaults: { ...policy.defaults, [bucket]: tier } });
  };

  const addException = () => {
    onChange({
      ...policy,
      exceptions: [...policy.exceptions, { title: "", tier: "ask" }],
    });
  };

  const updateException = (index: number, patch: Partial<{ title: string; tier: PolicyTier }>) => {
    onChange({
      ...policy,
      exceptions: policy.exceptions.map((e, i) => (i === index ? { ...e, ...patch } : e)),
    });
  };

  const removeException = (index: number) => {
    onChange({ ...policy, exceptions: policy.exceptions.filter((_, i) => i !== index) });
  };

  const tierSelect = (value: PolicyTier, onValue: (tier: PolicyTier) => void, testId: string) => (
    <Select value={value} onValueChange={(v) => onValue(v as PolicyTier)}>
      <SelectTrigger size="sm" className="w-28" data-testid={testId}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {(Object.keys(TIER_KEYS) as PolicyTier[]).map((tier) => (
          <SelectItem key={tier} value={tier} data-testid={testId + "-" + tier}>
            {t(TIER_KEYS[tier])}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );

  return (
    <div className="flex flex-col gap-3" data-testid="contacts-policy-editor">
      <p className="text-sm font-medium">{t("contacts.policy.defaultTitle")}</p>
      {POLICY_KIND_BUCKETS.map((bucket) => (
        <div key={bucket} className="flex items-center justify-between gap-2 text-sm">
          <span>{t(BUCKET_KEYS[bucket])}</span>
          {tierSelect(policy.defaults[bucket] ?? "ask", (tier) => setDefault(bucket, tier), "contacts-policy-default-" + bucket)}
        </div>
      ))}
      <div className="mt-1 flex items-center justify-between gap-2">
        <p className="text-sm font-medium">{t("contacts.policy.exceptionTitle")}</p>
        <Button type="button" variant="outline" size="sm" onClick={addException} data-testid="contacts-policy-exception-add">
          {t("contacts.policy.addException")}
        </Button>
      </div>
      {policy.exceptions.map((exception, index) => (
        <div key={index} className="flex items-center gap-2" data-testid={"contacts-policy-exception-" + index}>
          <Input
            className="h-8 flex-1 text-xs"
            value={exception.title}
            onChange={(e) => updateException(index, { title: e.target.value })}
            placeholder={t("contacts.policy.exceptionPlaceholder")}
            aria-label={t("contacts.policy.exceptionPlaceholder")}
            data-testid={"contacts-policy-exception-title-" + index}
            autoComplete="off"
          />
          {tierSelect(exception.tier, (tier) => updateException(index, { tier }), "contacts-policy-exception-tier-" + index)}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => removeException(index)}
            data-testid={"contacts-policy-exception-remove-" + index}
          >
            {t("contacts.policy.remove")}
          </Button>
        </div>
      ))}
      <p className="text-muted-foreground text-xs">{t("contacts.policy.effectiveNote")}</p>
      <p className="text-muted-foreground text-xs">{t("contacts.policy.autoAllowNote")}</p>
    </div>
  );
}
