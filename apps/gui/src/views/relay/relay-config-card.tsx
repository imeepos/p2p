import { useEffect } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";
import type { Resolver } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { AsyncButton } from "@/components/feedback/async-button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  addrRowsField,
  fromRows,
  toRows,
  type AddressRow,
} from "@/views/shared/address-rules";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";
import {
  FactoryDefaultsNotice,
} from "@/views/shared/factory-defaults-notice";
import {
  FORM_VALIDATION_MARK,
  isFlowMark,
  errorText,
} from "@/views/shared/form-flow";
import { useUnsavedGuard } from "@/views/shared/unsaved-guard";

interface RelayFormValues {
  relayAddrs: AddressRow[];
}

const relaySchema = z.object({ relayAddrs: addrRowsField("addrDuplicate") });
const relayResolver = zodResolver(relaySchema) as unknown as Resolver<RelayFormValues>;

interface RelayConfigCardProps {
  relayAddrs: string[];
  onSave: (next: string[]) => Promise<void>;
}

// 中继地址配置卡：独立小表单复用共享列表编辑器，保存后重置脏状态。
export function RelayConfigCard({ relayAddrs, onSave }: RelayConfigCardProps) {
  const { t } = useTranslation();
  const form = useForm<RelayFormValues>({
    resolver: relayResolver,
    defaultValues: { relayAddrs: toRows(relayAddrs) },
  });
  const serialized = relayAddrs.join("\n");

  // 外部（保存回读/设置页修改）同步进表单；按序列化值比对避免无谓重置。
  useEffect(() => {
    form.reset({ relayAddrs: toRows(relayAddrs) });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [serialized]);

  // 地址列表脏状态注册路由守卫：离开中继页时统一弹确认，放弃则还原为磁盘值
  useUnsavedGuard("relay-addrs", {
    hasUnsaved: () => form.formState.isDirty,
    discard: () => form.reset({ relayAddrs: toRows(relayAddrs) }),
  });

  const submit = (): Promise<void> =>
    new Promise((resolve, reject) => {
      void form.handleSubmit(async (values) => {
        try {
          await onSave(fromRows(values.relayAddrs));
          form.reset(values);
          toastSuccess(t("relay.config.saved"));
          resolve();
        } catch (error) {
          reject(error);
        }
      }, () => reject(new Error(FORM_VALIDATION_MARK)))();
    });

  return (
    <Card className="col-span-12 h-full lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.config.title")}</CardTitle>
        <CardDescription>{t("relay.config.hint")}</CardDescription>
      </CardHeader>
      {/* flex-1：卡被拉伸时把 footer 钉到卡底，左右卡内容底边齐平（R3 R2） */}
      <CardContent className="flex flex-1 flex-col gap-3">
        <FormProvider {...form}>
          <AddressListEditor<RelayFormValues>
            control={form.control}
            name="relayAddrs"
            label={t("relay.config.label")}
            placeholder="192.168.1.10/u3403"
          />
          {/* FactoryDefaultsNotice 消费 useFormContext，必须在 Provider 内；
              挂在外面拿到 null 的 control，解构即整页崩溃 */}
          <FactoryDefaultsNotice name="relayAddrs" />
        </FormProvider>
      </CardContent>
      <CardFooter className="justify-end border-t">
        <AsyncButton
          type="button"
          size="sm"
          disabled={!form.formState.isDirty}
          action={submit}
          onError={(error) => {
            if (isFlowMark(error, FORM_VALIDATION_MARK)) return;
            console.error("[relay] relayAddrs 保存失败", error);
            toastError(t("relay.config.saveFailed"), {
              description: errorText(error),
              context: "relay.relayAddrs_save",
            });
          }}
        >
          {t("relay.config.save")}
        </AsyncButton>
      </CardFooter>
    </Card>
  );
}