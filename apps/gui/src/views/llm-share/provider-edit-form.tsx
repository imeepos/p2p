import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

import { focusFirstInvalidField } from "./focus-first-error";
import {
  EMPTY_PROVIDER_FORM,
  newProviderId,
  parseProviderForm,
  type ProviderErrors,
  type ProviderFormValues,
} from "./provider-configs";
import type { ProviderConfig } from "./provider-configs";

function ProviderFieldError({ messageKey, htmlId }: { messageKey?: I18nKey; htmlId: string }) {
  const { t } = useTranslation();
  if (!messageKey) return null;
  return (
    <p role="alert" id={htmlId} className="text-destructive text-xs">
      {t(messageKey)}
    </p>
  );
}

// R2-12：字段 id → 错误键映射（聚焦顺序 = 表单视觉顺序）
function providerFieldErrorOf(errors: ProviderErrors): Partial<Record<string, I18nKey>> {
  return {
    "llm-provider-name": errors.name,
    "llm-provider-base-url": errors.baseUrl,
    "llm-provider-api-key": errors.apiKey,
    "llm-provider-models": errors.models,
  };
}

interface ProviderEditFormProps {
  editing: ProviderConfig | null;
  onSave: (config: ProviderConfig) => void;
  onCancel: () => void;
}

// 新增/编辑一条本地 provider 配置：名称 + OpenAI 兼容 baseUrl + apiKey + 模型清单。
// apiKey 仅存 localStorage（本机自用），列表展示一律走掩码。
export function ProviderEditForm({ editing, onSave, onCancel }: ProviderEditFormProps) {
  const { t } = useTranslation();
  const [values, setValues] = useState<ProviderFormValues>(
    editing
      ? {
          name: editing.name,
          baseUrl: editing.baseUrl,
          apiKey: editing.apiKey,
          modelsText: editing.models.join(", "),
        }
      : EMPTY_PROVIDER_FORM,
  );
  const [errors, setErrors] = useState<ProviderErrors>({});

  const set = (field: keyof ProviderFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    const parsed = parseProviderForm(values);
    setErrors(parsed.errors);
    const fieldErrors = providerFieldErrorOf(parsed.errors);
    focusFirstInvalidField(Object.keys(fieldErrors), (id) => fieldErrors[id] != null);
    if (!parsed.config) return;
    onSave({
      id: editing?.id ?? newProviderId(),
      createdAt: editing?.createdAt ?? Date.now(),
      ...parsed.config,
    });
  };

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={handleSubmit}
      noValidate
      data-testid="provider-edit-form"
    >
      <div className="grid grid-cols-2 gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="llm-provider-name">{t("llmShare.providers.formName")}</Label>
          <Input
            id="llm-provider-name"
            value={values.name}
            onChange={(e) => set("name")(e.target.value)}
            placeholder={t("llmShare.providers.formNamePlaceholder")}
            aria-invalid={errors.name ? true : undefined}
            aria-describedby={errors.name ? "llm-provider-name-error" : undefined}
          />
          <ProviderFieldError messageKey={errors.name} htmlId="llm-provider-name-error" />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="llm-provider-base-url">{t("llmShare.providers.formBaseUrl")}</Label>
          <Input
            id="llm-provider-base-url"
            value={values.baseUrl}
            onChange={(e) => set("baseUrl")(e.target.value)}
            placeholder={t("llmShare.providers.formBaseUrlPlaceholder")}
            aria-invalid={errors.baseUrl ? true : undefined}
            aria-describedby={errors.baseUrl ? "llm-provider-base-url-error" : undefined}
          />
          <ProviderFieldError messageKey={errors.baseUrl} htmlId="llm-provider-base-url-error" />
        </div>
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="llm-provider-api-key">{t("llmShare.providers.formApiKey")}</Label>
        <Input
          id="llm-provider-api-key"
          type="password"
          value={values.apiKey}
          onChange={(e) => set("apiKey")(e.target.value)}
          placeholder={t("llmShare.providers.formApiKeyPlaceholder")}
          aria-invalid={errors.apiKey ? true : undefined}
          aria-describedby={errors.apiKey ? "llm-provider-api-key-error" : undefined}
        />
        <ProviderFieldError messageKey={errors.apiKey} htmlId="llm-provider-api-key-error" />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="llm-provider-models">{t("llmShare.providers.formModels")}</Label>
        <Textarea
          id="llm-provider-models"
          rows={2}
          value={values.modelsText}
          onChange={(e) => set("modelsText")(e.target.value)}
          placeholder={t("llmShare.providers.formModelsPlaceholder")}
          aria-invalid={errors.models ? true : undefined}
          aria-describedby={errors.models ? "llm-provider-models-error" : undefined}
        />
        <ProviderFieldError messageKey={errors.models} htmlId="llm-provider-models-error" />
      </div>
      <div className="flex gap-2">
        <Button type="submit" size="sm">
          {t("llmShare.providers.save")}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={onCancel}>
          {t("llmShare.providers.cancel")}
        </Button>
      </div>
    </form>
  );
}
