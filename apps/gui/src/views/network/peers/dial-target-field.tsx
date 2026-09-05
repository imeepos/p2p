import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface DialTargetFieldProps {
  target: string;
  onTargetChange: (value: string) => void;
  invalid: boolean;
  commandError: string | null;
}

// 拨号目标输入：§6 格式校验失败内联红字，命令错误同样内联展示。
export function DialTargetField({
  target,
  onTargetChange,
  invalid,
  commandError,
}: DialTargetFieldProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor="dial-target">{t("peers.dial.targetLabel")}</Label>
      <Input
        id="dial-target"
        value={target}
        onChange={(event) => onTargetChange(event.target.value)}
        placeholder={t("peers.dial.targetPlaceholder")}
        className="font-mono text-xs"
        spellCheck={false}
        autoComplete="off"
        aria-invalid={invalid}
      />
      {invalid && (
        <p className="text-destructive text-xs" role="alert">
          {t("peers.dial.invalidFormat")}
        </p>
      )}
      {commandError && (
        <p className="text-destructive text-xs" role="alert">
          {commandError}
        </p>
      )}
    </div>
  );
}
