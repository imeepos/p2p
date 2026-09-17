import { useEffect } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useAuthzStore } from "@/stores/authz-store";
import { AddressListEditor } from "@/views/shared/address-list-editor";
import { FactoryDefaultsNotice } from "@/views/shared/factory-defaults-notice";
import type { SettingsFormValues } from "./config-schema";
import { SettingsBlock, SettingsGroup, SettingsRow } from "./settings-row";

// Radix Select forbids an empty item value; the sentinel maps to ""
// (= auto-binding disabled, config-centralization contract).
const NONE_SENTINEL = "__none__";

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

// Default friend role: catalog from the shared authz role list ipc;
// persistence goes through the whole-config save (authz_default_role_save
// writes the same field, so semantics stay identical to the contacts page).
function DefaultRoleRow() {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext<SettingsFormValues>();
  const role = useWatch({ control, name: "authzDefaultRole" });
  const roles = useAuthzStore((s) => s.roles);
  const loadError = useAuthzStore((s) => s.loadError);
  const loadAll = useAuthzStore((s) => s.loadAll);

  useEffect(() => {
    if (roles.length === 0 && loadError == null) void loadAll();
  }, [roles.length, loadError, loadAll]);

  return (
    <SettingsRow
      htmlFor="settings-default-role"
      label={t("contacts.authz.defaultRoleLabel")}
      description={t("contacts.authz.defaultRoleHint")}
      error={
        loadError != null ? (
          <p
            role="alert"
            className="text-destructive text-xs"
            data-testid="settings-default-role-error"
          >
            {t("contacts.authz.loadFailed")}
          </p>
        ) : null
      }
      control={
        <Select
          value={role === "" ? NONE_SENTINEL : role}
          onValueChange={(next) =>
            setValue("authzDefaultRole", next === NONE_SENTINEL ? "" : next, {
              shouldDirty: true,
            })
          }
        >
          <SelectTrigger
            id="settings-default-role"
            className="w-56"
            data-testid="settings-default-role"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={NONE_SENTINEL}>
              {t("contacts.authz.defaultRoleNone")}
            </SelectItem>
            {roles.map((item) => (
              <SelectItem key={item.roleId} value={item.roleId}>
                {item.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      }
    />
  );
}

// Settings entry for the three GuiConfig fields that were previously edited
// only on business pages (discovery/relay/contacts). Persistence still goes
// through the shared whole-config save bar; business page entries stay.
export function BootstrapRelayCard() {
  const { t } = useTranslation();

  return (
    <SettingsGroup
      title={t("settings.network.cardTitle")}
      description={t("settings.network.cardHint")}
    >
      <BootstrapBlock />
      <RelayBlock />
      <DefaultRoleRow />
    </SettingsGroup>
  );
}
