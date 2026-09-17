import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { Resolver } from "react-hook-form";

import type { FtpConfigView, FtpConfigSaveInput, GuiConfig } from "@/lib/ipc-types";
import {
  addrRowsField,
  fromRows,
  isValidIpv4,
  isValidTransportAddr,
  loopbackSocketRowsField,
  observationPortField,
  portField,
  toRows,
  type AddressRow,
} from "@/views/shared/address-rules";

// FTP 账号行：existing 标记该行来自 ftp_config_get 的既有用户（后端不回显
// 密码，既有行密码留空 = 保存发空串 = 后端保留原密码）；新增行必须显式
// 设置密码（新用户无现有密码可保留）。
export interface FtpAccountRow {
  user: string;
  password: string;
  existing: boolean;
}

const ftpAccountRowField = z
  .object({
    user: z.string().trim().min(1, "ftpUserRequired"),
    password: z.string(),
    existing: z.boolean(),
  })
  .superRefine((row, ctx) => {
    if (!row.existing && row.password.length === 0) {
      ctx.addIssue({
        code: "custom",
        path: ["password"],
        message: "ftpPasswordRequired",
      });
    }
  });

const ftpAccountsField = z
  .array(ftpAccountRowField)
  .refine(
    (rows) => new Set(rows.map((row) => row.user.trim())).size === rows.length,
    "ftpUserDuplicate",
  );

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

// rdFps 合法域 1..=60；空输入（NaN）归一为 undefined，统一落 fpsRange 文案。
export const fpsField = z.preprocess(
  (value) =>
    typeof value === "number" && Number.isNaN(value) ? undefined : value,
  z
    .number({ message: "fpsRange" })
    .int("fpsRange")
    .min(1, "fpsRange")
    .max(60, "fpsRange"),
);

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
  lanOnly: boolean;
  // 契约 §18.3 加法：设置页暂无 UI，仅随表单往返保真（防 config_save 整包
  // 覆写丢字段）；编辑入口在通讯录好友角色对话框（authz_default_role_save）。
  authzDefaultRole: string;
  // config-centralization W1：远程访问默认策略三字段（远程访问区编辑）。
  rdRequireApproval: boolean;
  rdFps: number;
  tunnelServeAllow: AddressRow[];
  // config-centralization W2b：FTP 服务配置（服务区编辑，ftp_config_save 持久化，
  // 不属于 GuiConfig 整包——toGuiConfig 不携带，保存走 toFtpSaveInput 单独落盘）。
  ftpRoot: string;
  ftpAuthz: boolean;
  ftpAccounts: FtpAccountRow[];
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
  lanOnly: z.boolean(),
  authzDefaultRole: z.string(),
  rdRequireApproval: z.boolean(),
  rdFps: fpsField,
  tunnelServeAllow: loopbackSocketRowsField("addrDuplicate"),
  ftpRoot: z.string(),
  ftpAuthz: z.boolean(),
  ftpAccounts: ftpAccountsField,
})
  // FTP 未装配（未开鉴权且无账号）时 root 允许空——文件缺失回落空值可原样
  // 往返，不挡无关字段的保存；一旦启用（鉴权开或表内有账号）root 必填。
  .superRefine((values, ctx) => {
    const provisioned = values.ftpAuthz || values.ftpAccounts.length > 0;
    if (provisioned && values.ftpRoot.trim().length === 0) {
      ctx.addIssue({
        code: "custom",
        path: ["ftpRoot"],
        message: "ftpRootRequired",
      });
    }
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
  lanOnly: false,
  authzDefaultRole: "friend",
  rdRequireApproval: true,
  rdFps: 15,
  tunnelServeAllow: [],
  ftpRoot: "",
  ftpAuthz: false,
  ftpAccounts: [],
};

export function toFormValues(config: GuiConfig, ftp?: FtpConfigView): SettingsFormValues {
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
    lanOnly: config.lanOnly ?? false, // serde default: false when absent (v11 16.5)
    authzDefaultRole: config.authzDefaultRole ?? "friend", // serde default (§18.3)
    rdRequireApproval: config.rdRequireApproval ?? true, // serde default (W1 contract)
    rdFps: config.rdFps ?? 15, // serde default (W1 contract, valid range 1..=60)
    tunnelServeAllow: toRows(config.tunnelServeAllow ?? []), // serde default: empty
    // W2b：ftp 缺省 = 未装配（root 空 + 校验挡保存），与后端「文件缺失回落空
    // 值」口径一致；既有用户行 existing=true，密码一律不回显留空。
    ftpRoot: ftp?.root ?? "",
    ftpAuthz: ftp?.authz ?? false,
    ftpAccounts:
      ftp?.users.map((user) => ({ user, password: "", existing: true })) ?? [],
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
    lanOnly: values.lanOnly,
    authzDefaultRole: values.authzDefaultRole,
    rdRequireApproval: values.rdRequireApproval,
    rdFps: values.rdFps,
    tunnelServeAllow: fromRows(values.tunnelServeAllow),
  };
}

// W2b 契约钉死的保存语义：accounts 为全量目标表；既有用户密码留空发空串
// （= 后端保留原密码），显式输入才覆盖；表内移除的用户由后端按全量语义删除。
export function toFtpSaveInput(values: SettingsFormValues): FtpConfigSaveInput {
  const accounts: Record<string, string> = {};
  for (const row of values.ftpAccounts) {
    accounts[row.user.trim()] = row.password;
  }
  return { root: values.ftpRoot.trim(), authz: values.ftpAuthz, accounts };
}
