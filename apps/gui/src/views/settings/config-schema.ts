import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { Resolver } from "react-hook-form";

import type { GuiConfig } from "@/lib/ipc-types";
import {
  addrRowsField,
  fromRows,
  isValidIpv4,
  isValidTransportAddr,
  observationPortField,
  portField,
  toRows,
  type AddressRow,
} from "@/views/shared/address-rules";

// 观测地址契约语法为 socket（ip:port，gui-contract.md §3）；transport 语法
// 兼容既有输入。出厂默认观测端点为 socket 语法，恢复默认后必须通过校验。
function isValidObservationAddr(value: string): boolean {
  const matched = value.match(/^(\d{1,3}(?:\.\d{1,3}){3}):(\d{1,5})$/);
  if (matched) {
    const port = Number(matched[2]);
    return isValidIpv4(matched[1]) && port >= 1 && port <= 65535;
  }
  return isValidTransportAddr(value);
}

const observationRowField = z.object({
  value: z
    .string()
    .min(1, "addrRequired")
    .refine(isValidObservationAddr, "addrFormat"),
});

function observationRowsField() {
  return z
    .array(observationRowField)
    .refine(
      (rows) => new Set(rows.map((row) => row.value)).size === rows.length,
      "addrDuplicate",
    );
}

// useFieldArray 要求数组元素为对象，地址列表以 { value } 行编辑，出入转换。
export interface SettingsFormValues {
  quicPort: number;
  tcpPort: number;
  enableMdns: boolean;
  dataDir: string;
  bootstrap: AddressRow[];
  relayAddrs: AddressRow[];
  advertisedAddrs: AddressRow[];
  observationPort: number | null;
  observationAddrs: AddressRow[];
}

export const settingsSchema = z.object({
  quicPort: portField,
  tcpPort: portField,
  enableMdns: z.boolean(),
  dataDir: z.string(),
  bootstrap: addrRowsField("addrDuplicate"),
  relayAddrs: addrRowsField("addrDuplicate"),
  advertisedAddrs: addrRowsField("addrDuplicate"),
  observationPort: observationPortField,
  observationAddrs: observationRowsField(),
});

// z.preprocess 的输入类型与表单值不同，此处收口为 Resolver。
export const settingsResolver = zodResolver(settingsSchema) as unknown as Resolver<SettingsFormValues>;

export const EMPTY_SETTINGS: SettingsFormValues = {
  quicPort: 0,
  tcpPort: 0,
  enableMdns: true,
  dataDir: "",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

export function toFormValues(config: GuiConfig): SettingsFormValues {
  return {
    quicPort: config.quicPort,
    tcpPort: config.tcpPort,
    enableMdns: config.enableMdns,
    dataDir: config.dataDir,
    bootstrap: toRows(config.bootstrap),
    relayAddrs: toRows(config.relayAddrs),
    advertisedAddrs: toRows(config.advertisedAddrs),
    observationPort: config.observationPort,
    observationAddrs: toRows(config.observationAddrs),
  };
}

export function toGuiConfig(values: SettingsFormValues): GuiConfig {
  return {
    quicPort: values.quicPort,
    tcpPort: values.tcpPort,
    enableMdns: values.enableMdns,
    dataDir: values.dataDir,
    bootstrap: fromRows(values.bootstrap),
    relayAddrs: fromRows(values.relayAddrs),
    advertisedAddrs: fromRows(values.advertisedAddrs),
    observationPort: values.observationPort,
    observationAddrs: fromRows(values.observationAddrs),
  };
}
