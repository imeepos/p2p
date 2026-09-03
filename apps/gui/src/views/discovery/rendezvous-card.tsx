import { NetworkIcon, Trash2Icon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { useConfirm } from "@/components/feedback/confirm-provider";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { AddAddressDialog } from "./add-address-dialog";
import { ACTION_CANCELLED_MARK } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";

interface RendezvousCardProps {
  bootstrap: string[];
  onChange: (next: string[]) => Promise<boolean>;
}

function RendezvousTable({
  bootstrap,
  onDelete,
}: {
  bootstrap: string[];
  onDelete: (addr: string) => Promise<void>;
}) {
  const { t } = useTranslation();

  return (
    <Table containerClassName="max-h-80 overflow-y-auto">
      <TableHeader className="[&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-card">
        <TableRow>
          <TableHead>{t("common.labels.address")}</TableHead>
          <TableHead className="w-12" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {bootstrap.map((addr) => (
          <TableRow key={addr}>
            <TableCell className="font-mono text-xs">{addr}</TableCell>
            <TableCell>
              <AsyncButton
                variant="ghost"
                size="icon"
                iconOnly
                aria-label={t("discovery.rendezvous.deleteAction")}
                action={() => onDelete(addr)}
                onError={() => {
                  // 取消/上游已 toast：fail 图标即反馈，不重复弹错
                }}
              >
                <Trash2Icon aria-hidden />
              </AsyncButton>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

// rendezvous 地址簿：增删走持久化配置；手动注册/查询未暴露给 GUI（裁决：
// 底座能力为 pub(crate)，CLI 已覆盖），节点仍经 mDNS/rendezvous 自动发现。
export function RendezvousCard({ bootstrap, onChange }: RendezvousCardProps) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [saving, setSaving] = useState(false);

  const addAddress = async (addr: string): Promise<boolean> => {
    setSaving(true);
    try {
      return await onChange([...bootstrap, addr]);
    } finally {
      setSaving(false);
    }
  };

  const removeAddr = async (addr: string): Promise<void> => {
    const ok = await confirm({
      title: t("discovery.rendezvous.deleteTitle"),
      description: t("discovery.rendezvous.deleteDesc", { addr }),
      confirmText: t("discovery.rendezvous.deleteAction"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) throw new Error(ACTION_CANCELLED_MARK);
    const saved = await onChange(bootstrap.filter((item) => item !== addr));
    if (!saved) throw new Error("bootstrap save failed");
  };

  return (
    <Card className="col-span-12 lg:col-span-8">
      <CardHeader>
        <CardTitle>{t("discovery.rendezvous.title")}</CardTitle>
        <CardDescription>{t("discovery.rendezvous.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {bootstrap.length === 0 ? (
          <EmptyState
            icon={NetworkIcon}
            title={t("discovery.rendezvous.empty")}
            description={t("discovery.rendezvous.manualUnavailable")}
            action={
              <AddAddressDialog existing={bootstrap} saving={saving} onAdd={addAddress} />
            }
          />
        ) : (
          <RendezvousTable
            bootstrap={bootstrap}
            onDelete={removeAddr}
          />
        )}
        {bootstrap.length > 0 ? (
          <p className="text-muted-foreground text-xs">
            {t("discovery.rendezvous.manualUnavailable")}
          </p>
        ) : null}
        <AddAddressDialog existing={bootstrap} saving={saving} onAdd={addAddress} />
      </CardContent>
    </Card>
  );
}