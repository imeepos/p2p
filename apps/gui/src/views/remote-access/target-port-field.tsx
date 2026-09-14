import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

// 通用/被访两侧共用的端口输入行：固定 127.0.0.1: 前缀 + 端口段输入。
export function TargetPortField({
  inputId,
  label,
  placeholder,
  hint,
  value,
  onChange,
}: {
  inputId: string;
  label: string;
  placeholder: string;
  hint: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={inputId}>{label}</Label>
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground font-mono text-sm">127.0.0.1:</span>
        <Input
          id={inputId}
          inputMode="numeric"
          value={value}
          placeholder={placeholder}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1"
        />
      </div>
      <p className="text-muted-foreground text-xs">{hint}</p>
    </div>
  );
}
