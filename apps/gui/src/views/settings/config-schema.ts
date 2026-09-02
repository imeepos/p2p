import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { Resolver } from "react-hook-form";

import type { GuiConfig } from "@/lib/ipc-types";
import {
  addrRowsField,
  fromRows,
  observationPortField,
  portField,
  toRows,
  type AddressRow,
} from "@/views/shared/address-rules";

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
  observationAddrs: addrRowsField("addrDuplicate"),
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
