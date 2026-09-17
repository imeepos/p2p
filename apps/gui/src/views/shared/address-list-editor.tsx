import { PlusIcon, Trash2Icon } from "lucide-react";
import {
  useFieldArray,
  useFormContext,
  type Control,
  type FieldArrayPath,
  type FieldValues,
} from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ErrorText } from "@/views/shared/error-text";

interface RowError {
  message?: string;
}

function rowMessage(container: unknown, index: number): string | undefined {
  if (!Array.isArray(container)) return undefined;
  // 对象行（{ value }）的行级校验消息落在 row.value.message（zod 路径
  // addrs.<i>.value）；直接挂 row.message 的形态保留兼容。
  const row = container[index] as
    | (RowError & { value?: RowError })
    | undefined;
  return row?.message ?? row?.value?.message;
}

function rootMessage(container: unknown): string | undefined {
  if (!container || Array.isArray(container)) return undefined;
  return (container as { root?: RowError }).root?.message;
}

interface AddressListEditorProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldArrayPath<T>;
  label: string;
  hint?: string;
  placeholder?: string;
  // Optional async gate before row removal (delete confirmation, etc.);
  // resolve false keeps the row. Absent = immediate removal (legacy callers).
  confirmRemove?: (index: number) => Promise<boolean>;
}

// 地址列表行编辑器：bootstrap/relay/advertised/observation 共用，
// 行内联红字校验，数组级错误（重复项）落在 root。
export function AddressListEditor<T extends FieldValues>({
  control,
  name,
  label,
  hint,
  placeholder,
  confirmRemove,
}: AddressListEditorProps<T>) {
  const { t } = useTranslation();
  const {
    register,
    formState: { errors },
  } = useFormContext<T>();
  const { fields, append, remove } = useFieldArray({ control, name });
  const container = errors[name] as unknown;

  const removeRow = async (index: number): Promise<void> => {
    if (confirmRemove != null && !(await confirmRemove(index))) return;
    remove(index);
  };

  return (
    // data-field 供 focusFirstInvalidField 定位（`[data-field="name"] input`）
    <div className="flex flex-col gap-2" data-field={name}>
      <div className="flex flex-col gap-0.5">
        <Label>{label}</Label>
        {hint ? <p className="text-muted-foreground text-xs">{hint}</p> : null}
      </div>
      {fields.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("common.addressList.empty")}
        </p>
      ) : (
        fields.map((field, index) => {
          // F13：行级可见标签（组标题+序号），占位符只承担示例职责
          const inputId = `${name}-row-${index}`;
          return (
          <div key={field.id} className="flex flex-col gap-1">
            <Label
              htmlFor={inputId}
              className="text-muted-foreground text-xs"
            >
              {t("common.addressList.rowLabel", { index: index + 1 })}
            </Label>
            <div className="flex items-center gap-2">
              <Input
                id={inputId}
                className="font-mono text-xs"
                placeholder={placeholder}
                {...register(`${name}.${index}.value` as never)}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t("common.addressList.remove")}
                onClick={() => void removeRow(index)}
              >
                <Trash2Icon aria-hidden />
              </Button>
            </div>
            <ErrorText code={rowMessage(container, index)} />
          </div>
          );
        })
      )}
      <ErrorText code={rootMessage(container)} />
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="w-fit"
        onClick={() => append({ value: "" } as never)}
      >
        <PlusIcon aria-hidden />
        {t("common.addressList.add")}
      </Button>
    </div>
  );
}