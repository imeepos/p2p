import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useNodeStore } from "@/stores/node-store";
import type { I18nKey } from "@/i18n/types";
import type { SettingsFormValues } from "./config-schema";
import { SettingsGroup, SettingsRow } from "./settings-row";

// 从监听地址提取实际生效端口：QUIC 记 /端口 或 /u端口，TCP 固定 /t端口。
function effectivePort(listenAddrs: string[], tcp: boolean): number | null {
  const pattern = tcp ? /\/t(\d+)$/ : /\/(?:u)?(\d+)$/;
  for (const addr of listenAddrs) {
    const matched = addr.match(pattern);
    if (matched) return Number(matched[1]);
  }
  return null;
}

interface PortFieldProps {
  name: "quicPort" | "tcpPort";
  htmlId: string;
  label: string;
  effective: number | null;
}

// 端口行：0（随机）不裸显，输入框置空并展示随机端口语义；节点运行中
// 就近展示当前实际生效端口。校验失败就地 role=alert 提示（失焦触发，
// 已有错误随输入复验即改即消），口径同 dial-target-field 的 F14 实现。
function PortField({ name, htmlId, label, effective }: PortFieldProps) {
  const { t } = useTranslation();
  const {
    control,
    setValue,
    trigger,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  const value = useWatch({ control, name });
  const isRandom = value === 0 || value == null;
  const errorCode = errors[name]?.message;
  const errorId = `${htmlId}-error`;

  return (
    <SettingsRow
      htmlFor={htmlId}
      label={label}
      description={
        <>
          {isRandom ? (
            <span className="block">{t("settings.network.randomPortHint")}</span>
          ) : null}
          {effective !== null ? (
            <span className="block">
              {t("settings.network.effectivePort", { port: effective })}
            </span>
          ) : null}
        </>
      }
      error={
        errorCode != null ? (
          <p
            id={errorId}
            role="alert"
            className="text-destructive text-xs"
            data-testid={errorId}
          >
            {t(`common.validation.${errorCode}` as I18nKey)}
          </p>
        ) : null
      }
      control={
        <Input
          id={htmlId}
          type="number"
          inputMode="numeric"
          min={0}
          max={65535}
          className="w-40"
          placeholder={t("settings.network.randomPortPlaceholder")}
          value={isRandom ? "" : String(value)}
          aria-invalid={errorCode != null ? true : undefined}
          aria-describedby={errorCode != null ? errorId : undefined}
          onChange={(event) => {
            const parsed = Number(event.target.value);
            const next =
              event.target.value === "" || Number.isNaN(parsed) ? 0 : parsed;
            setValue(name, next, { shouldDirty: true });
            if (errors[name] != null) void trigger(name);
          }}
          onBlur={() => void trigger(name)}
        />
      }
    />
  );
}

// 网络组：quic/tcp 端口（0 = 随机）与 mDNS / 局域网开关。
export function NetworkCard() {
  const { t } = useTranslation();
  const { setValue } = useFormContext<SettingsFormValues>();
  const enableMdns = useWatch({ name: "enableMdns" });
  const lanOnly = useWatch({ name: "lanOnly" });
  const status = useNodeStore((s) => s.status);
  const listenAddrs = status?.running ? status.listenAddrs : [];

  return (
    <SettingsGroup
      title={t("settings.cards.network")}
      description={t("settings.network.hint")}
    >
      <PortField
        name="quicPort"
        htmlId="settings-quic-port"
        label={t("settings.network.quicPort")}
        effective={effectivePort(listenAddrs, false)}
      />
      <PortField
        name="tcpPort"
        htmlId="settings-tcp-port"
        label={t("settings.network.tcpPort")}
        effective={effectivePort(listenAddrs, true)}
      />
      <SettingsRow
        htmlFor="settings-mdns"
        label={t("settings.network.mdns")}
        description={t("settings.network.mdnsHint")}
        control={
          <Switch
            id="settings-mdns"
            checked={enableMdns}
            onCheckedChange={(next) =>
              setValue("enableMdns", next, { shouldDirty: true })
            }
          />
        }
      />
      <SettingsRow
        htmlFor="settings-lan-only"
        label={t("settings.network.lanOnly")}
        description={t("settings.network.lanOnlyHint")}
        control={
          <Switch
            id="settings-lan-only"
            checked={lanOnly}
            onCheckedChange={(next) =>
              setValue("lanOnly", next, { shouldDirty: true })
            }
          />
        }
      />
    </SettingsGroup>
  );
}
