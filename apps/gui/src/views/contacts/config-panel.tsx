import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { toastError } from "@/components/feedback/toast";
import { errorText } from "@/views/shared/form-flow";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { I18nKey } from "@/i18n/types";
import { useAcpStore } from "@/acp/acp-store";
import type { ConfigOption } from "@/acp/protocol";

/** 语义类别名（model/thought_level 等）；未识别类别回退 agent 声明名，不硬编码目录 */
const CATEGORY_KEY: Record<string, I18nKey> = {
  model: "acp.config.category.model",
  model_config: "acp.config.category.model_config",
  thought_level: "acp.config.category.thought_level",
};

function optionLabel(t: (key: I18nKey) => string, option: ConfigOption): string {
  const key = option.category ? CATEGORY_KEY[option.category] : undefined;
  return key ? t(key) : option.name;
}

/** 单个配置行：select 走下拉、boolean 走开关，均纳入既有配置下发链路；
 * 未识别 type 按 ACP 契约忽略（agent 会用默认值继续） */
function ConfigRow({ option }: { option: ConfigOption }) {
  const { t } = useTranslation();
  const setConfigOption = useAcpStore((s) => s.setConfigOption);
  // P2#4 乐观显示值：下发期间即显新值；失败回滚 store 权威值并 toast，
  // 绝不静默留在改动后的假值上。
  const [optimistic, setOptimistic] = useState<string | boolean | null>(null);
  const current = optimistic ?? option.currentValue;

  const apply = (value: string | boolean) => {
    setOptimistic(value);
    void Promise.resolve(setConfigOption(option.id, value))
      .catch((error) => {
        console.error("[contacts] 配置下发失败", option.id, error);
        toastError(t("contacts.config.applyFailed"), {
          description: errorText(error),
          context: "acp_config_set",
        });
      })
      .finally(() => setOptimistic(null));
  };

  if (option.type === "boolean") {
    const labelId = "acp-config-label-" + option.id;
    const checked = current === true;
    return (
      <div className="flex items-center justify-between gap-2 text-sm">
        <span id={labelId}>{optionLabel(t, option)}</span>
        <Switch checked={checked} onCheckedChange={(v) => apply(v)}
          aria-labelledby={labelId}
          data-testid={"acp-config-option-" + option.id} />
      </div>
    );
  }
  if (option.type !== "select" || !option.options || option.options.length === 0) return null;
  const labelId = "acp-config-label-" + option.id;
  return (
    <div className="flex items-center justify-between gap-2 text-sm">
      <span id={labelId}>{optionLabel(t, option)}</span>
      <Select value={String(current)}
        onValueChange={(v) => apply(v)}>
        <SelectTrigger size="sm" className="w-44" aria-labelledby={labelId}
          data-testid={"acp-config-option-" + option.id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {option.options.map((choice) => (
            <SelectItem key={choice.value} value={choice.value}
              data-testid={"acp-config-choice-" + option.id + "-" + choice.value}>
              {choice.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

export function ConfigPanel() {
  const { t } = useTranslation();
  const configOptions = useAcpStore((s) =>
    s.activeSessionId ? s.interactions[s.activeSessionId]?.configOptions : undefined,
  );
  if (!configOptions || configOptions.length === 0) return null;
  return (
    <Card data-testid="acp-config-panel">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{t("acp.config.card")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {configOptions.map((option) => (
          <ConfigRow key={option.id} option={option} />
        ))}
      </CardContent>
    </Card>
  );
}