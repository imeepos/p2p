import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { EntityCombobox } from "@/components/picker";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

import { usePeerPickerOptions } from "./peer-options";

interface PeerIdFieldProps {
  label: string;
  /** 输入框 id（Label htmlFor/aria-describedby 定位用） */
  inputId: string;
  value: string;
  onValueChange: (value: string) => void;
  onBlur: () => void;
  /** 即时校验错误（t 键）：由父级按 touched/提交状态计算 */
  errorKey: I18nKey | null;
  errorId: string;
  placeholder?: string;
}

// R2-05：PeerId 关联输入 = 全站节点选择器（EntityCombobox）+ 自由文本兜底，
// base58/32 字节即时校验（F14 时机：首次失焦前不打断输入）。
export function PeerIdField({
  label,
  inputId,
  value,
  onValueChange,
  onBlur,
  errorKey,
  errorId,
  placeholder,
}: PeerIdFieldProps) {
  const { t } = useTranslation();
  const options = usePeerPickerOptions();
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={inputId + "-pick"}>{t("picker.friendPickLabel")}</Label>
      <EntityCombobox
        id={inputId + "-pick"}
        testId={inputId + "-pick"}
        options={options}
        value={options.some((option) => option.value === value) ? value : null}
        onChange={(next) => {
          if (next) onValueChange(next);
        }}
      />
      <Label htmlFor={inputId}>{label}</Label>
      <Input
        id={inputId}
        value={value}
        onChange={(event) => onValueChange(event.target.value)}
        onBlur={onBlur}
        placeholder={placeholder}
        className="font-mono text-xs"
        spellCheck={false}
        autoComplete="off"
        aria-invalid={errorKey ? true : undefined}
        aria-describedby={errorKey ? errorId : undefined}
      />
      {errorKey ? (
        <p role="alert" id={errorId} className="text-destructive text-xs">
          {t(errorKey)}
        </p>
      ) : null}
    </div>
  );
}
