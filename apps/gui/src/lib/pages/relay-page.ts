// relay 页 descriptor：保存中继地址与中继配置卡保存按钮同源（address-rules
// 校验 -> configSave，校验失败零写入）；state 水位/逐跳计数与水位卡、逐跳
// 统计卡同源读 store metrics。
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { isValidTransportAddr, noDuplicateAddrs } from "@/views/shared/address-rules";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "relay",
  description: "中继页：relay 地址配置与会话水位观测",
  actions: [
    {
      name: "saveRelayAddrs",
      description: "保存中继地址列表（与中继配置卡保存按钮同源，格式 ip/u端口 或 ip/t端口）",
      args: [
        { name: "relayAddrs", type: "array", required: true, description: "传输地址字符串数组，去重校验" },
      ],
    },
  ],
};

function validateRelayAddrs(raw: unknown[]): string[] {
  const addrs = raw.map((item) => String(item).trim());
  for (const addr of addrs) {
    if (!isValidTransportAddr(addr)) throw new Error(`addrFormat: ${addr}`);
  }
  if (!noDuplicateAddrs(addrs)) throw new Error("addrDuplicate");
  return addrs;
}

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  switch (action) {
    case "saveRelayAddrs": {
      const raw = Array.isArray(args.relayAddrs) ? args.relayAddrs : [];
      const relayAddrs = validateRelayAddrs(raw);
      const config = await ipc.configGet();
      return ipc.configSave({ ...config, relayAddrs });
    }
    default:
      throw new Error(`relay 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const metrics = useNodeStore.getState().metrics;
  return {
    relaySessionsActive: metrics?.relaySessionsActive ?? null,
    relayReconnects: metrics?.relayReconnects ?? null,
    dialPunch: metrics ? { ok: metrics.dialPunchOk, fail: metrics.dialPunchFail } : null,
    dialRelay: metrics ? { ok: metrics.dialRelayOk, fail: metrics.dialRelayFail } : null,
  };
}

export const relayPage: PageEntry = { descriptor, execute, state };
