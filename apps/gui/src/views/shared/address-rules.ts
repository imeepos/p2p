import { z } from "zod";

// 校验错误一律输出稳定代码字符串，由视图经 i18n 渲染，保证双语同步。
// 语法对齐 crates/p2p-transport TransportAddr：ip/u端口（QUIC）、ip/t端口（TCP）。
const IPV4_RE = /^(\d{1,3})(\.\d{1,3}){3}$/;
const TRANSPORT_ADDR_RE = /^(.+)\/([ut])(\d{1,5})$/;
const IPV6_CHARS_RE = /^[0-9a-fA-F:]+$/;

export function isValidIpv4(value: string): boolean {
  if (!IPV4_RE.test(value)) return false;
  return value.split(".").every((part) => Number(part) <= 255);
}

function isValidIp(value: string): boolean {
  if (IPV4_RE.test(value)) return isValidIpv4(value);
  // IPv6 只做字符级校验（含 :: 缩写），精确解析交给后端，解析失败由命令 Err 上报。
  return value.includes(":") && IPV6_CHARS_RE.test(value);
}

export function isValidTransportAddr(value: string): boolean {
  const matched = value.match(TRANSPORT_ADDR_RE);
  if (!matched) return false;
  const [, ip, , portText] = matched;
  const port = Number(portText);
  return isValidIp(ip) && port >= 1 && port <= 65535;
}

export function noDuplicateAddrs(items: string[]): boolean {
  return new Set(items).size === items.length;
}

// 端口输入经 register valueAsNumber，空输入得 NaN，落到 portRequired。
export const portField = z
  .number({ message: "portRequired" })
  .int("portRange")
  .min(0, "portRange")
  .max(65535, "portRange");

// 观测端口可空：空输入（NaN）归一为 null。
export const observationPortField = z.preprocess(
  (value) => (typeof value === "number" && Number.isNaN(value) ? null : value),
  z
    .number({ message: "observationPortRange" })
    .int("observationPortRange")
    .min(1, "observationPortRange")
    .max(65535, "observationPortRange")
    .nullable(),
);

export const transportAddrField = z
  .string()
  .min(1, "addrRequired")
  .refine(isValidTransportAddr, "addrFormat");

// useFieldArray 不支持原始类型数组，行统一为 { value: string }。
export interface AddressRow {
  value: string;
}

export const addressRowField = z.object({ value: transportAddrField });

export function addrRowsField(duplicateCode: string) {
  return z
    .array(addressRowField)
    .refine((rows) => noDuplicateAddrs(rows.map((row) => row.value)), duplicateCode);
}

export const toRows = (items: string[]): AddressRow[] => items.map((value) => ({ value }));
export const fromRows = (rows: AddressRow[]): string[] => rows.map((row) => row.value);