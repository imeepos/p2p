import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import type { I18nKey } from "@/i18n/types";
import {
  FACTORY_LIST_DEFAULTS,
  FACTORY_LIST_HINT_KEYS,
  factoryRows,
  type FactoryListName,
} from "./factory-defaults";

interface FactoryDefaultsNoticeProps {
  name: FactoryListName;
}

// 地址列表空态：告知将生效的出厂端点（节点装配时空列表回落出厂默认，
// state.rs with_factory_fallback）；恢复按钮按用户点击才写入表单并置脏。
// 与宿主表单解耦：任何含同名地址行字段的 react-hook-form 表单均可挂载。
export function FactoryDefaultsNotice({ name }: FactoryDefaultsNoticeProps) {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext();
  const rows = (useWatch({ control, name }) ?? []) as { value: string }[];

  if (rows.length > 0) return null;

  const restore = () => {
    setValue(name, factoryRows(name), { shouldDirty: true });
  };

  return (
    <div className="flex flex-col gap-1">
      <p className="text-muted-foreground text-xs">
        {t(FACTORY_LIST_HINT_KEYS[name] as I18nKey, {
          addrs: FACTORY_LIST_DEFAULTS[name].join(", "),
        })}
      </p>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="w-fit"
        onClick={restore}
      >
        {t("settings.defaults.restore")}
      </Button>
    </div>
  );
}
