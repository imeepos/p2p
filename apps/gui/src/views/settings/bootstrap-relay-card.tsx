import { useFormContext } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { AddressListEditor } from "@/views/shared/address-list-editor";
import { FactoryDefaultsNotice } from "@/views/shared/factory-defaults-notice";
import type { SettingsFormValues } from "./config-schema";
import { SettingsBlock, SettingsGroup } from "./settings-row";

// Bootstrap address book: same GuiConfig field as the discovery page editor;
// row removal reuses the discovery delete confirmation (destructive).
function BootstrapBlock() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const { control, getValues } = useFormContext<SettingsFormValues>();

  const confirmRemove = async (index: number): Promise<boolean> => {
    const addr = getValues(`bootstrap.${index}.value`) ?? "";
    return confirm({
      title: t("discovery.rendezvous.deleteTitle"),
      description: t("discovery.rendezvous.deleteDesc", { addr }),
      confirmText: t("discovery.rendezvous.deleteAction"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
  };

  return (
    <SettingsBlock>
      <AddressListEditor
        control={control}
        name="bootstrap"
        label={t("discovery.rendezvous.title")}
        hint={t("discovery.rendezvous.hint")}
        placeholder="192.168.1.10/u3400"
        confirmRemove={confirmRemove}
      />
      <FactoryDefaultsNotice name="bootstrap" />
    </SettingsBlock>
  );
}

// Relay addresses: same GuiConfig field as the relay page editor, plain rows.
function RelayBlock() {
  const { t } = useTranslation();
  const { control } = useFormContext<SettingsFormValues>();

  return (
    <SettingsBlock>
      <AddressListEditor
        control={control}
        name="relayAddrs"
        label={t("relay.config.label")}
        hint={t("relay.config.hint")}
        placeholder="192.168.1.10/u3403"
      />
      <FactoryDefaultsNotice name="relayAddrs" />
    </SettingsBlock>
  );
}

// Settings entry for the GuiConfig fields that were previously edited only
// on business pages (discovery/relay). Persistence still goes through the
// shared whole-config save bar; business page entries stay. The default
// authz role moved to authz-card.tsx (2026-09-18 IA 重组).
export function BootstrapRelayCard() {
  const { t } = useTranslation();

  return (
    <SettingsGroup
      title={t("settings.network.cardTitle")}
      description={t("settings.network.cardHint")}
    >
      <BootstrapBlock />
      <RelayBlock />
    </SettingsGroup>
  );
}
