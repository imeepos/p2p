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
  const row = container[index] as RowError | undefined;
  return row?.message;
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
}

// 地址列表行编辑器：bootstrap/relay/advertised/observation 共用，
// 行内联红字校验，数组级错误（重复项）落在 root。
export function AddressListEditor<T extends FieldValues>({
  control,
  name,
  label,
  hint,
  placeholder,
}: AddressListEditorProps<T>) {
  const { t } = useTranslation();
  const {
    register,
    formState: { errors },
  } = useFormContext<T>();
  const { fields, append, remove } = useFieldArray({ control, name });
  const container = errors[name] as unknown;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-0.5">
        <Label>{label}</Label>
        {hint ? <p className="text-muted-foreground text-xs">{hint}</p> : null}
      </div>
      {fields.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("common.addressList.empty")}
        </p>
      ) : (
        fields.map((field, index) => (
          <div key={field.id} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <Input
                className="font-mono text-xs"
                placeholder={placeholder}
                {...register(`${name}.${index}.value` as never)}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t("common.addressList.remove")}
                onClick={() => remove(index)}
              >
                <Trash2Icon aria-hidden />
              </Button>
            </div>
            <ErrorText code={rowMessage(container, index)} />
          </div>
        ))
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