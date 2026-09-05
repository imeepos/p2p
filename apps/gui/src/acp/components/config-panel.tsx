import { useTranslation } from "react-i18next";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
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
  if (option.type === "boolean") {
    const labelId = "acp-config-label-" + option.id;
    const checked = option.currentValue === true;
    return (
      <div className="flex items-center justify-between gap-2 text-sm">
        <span id={labelId}>{optionLabel(t, option)}</span>
        <Switch checked={checked} onCheckedChange={(v) => void setConfigOption(option.id, v)}
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
      <Select value={String(option.currentValue)}
        onValueChange={(v) => void setConfigOption(option.id, v)}>
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