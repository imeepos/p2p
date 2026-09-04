// discovery 页 descriptor：mDNS 开关 / rendezvous 地址簿与页面同源
// （configGet -> 校验 -> configSave，校验复用 address-rules）；removeBootstrap
// 与删除确认框同源，为危险动作，registry 强制 args.confirm===true。
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { isValidTransportAddr } from "@/views/shared/address-rules";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "discovery",
  description: "发现页：mDNS 开关、rendezvous 地址簿与发现结果表",
  actions: [
    {
      name: "setMdns",
      description: "切换 mDNS 并持久化配置（与 mDNS 开关同源，节点运行中需重启生效）",
      args: [
        { name: "enable", type: "boolean", required: true, description: "true 开启 / false 关闭" },
      ],
    },
    {
      name: "addBootstrap",
      description: "添加 rendezvous 地址（与添加地址对话框同源，格式 ip/u端口 或 ip/t端口）",
      args: [
        { name: "addr", type: "string", required: true, description: "传输地址，如 192.168.1.10/u3400" },
      ],
    },
    {
      name: "removeBootstrap",
      description: "删除 rendezvous 地址（与删除确认框同源）",
      confirm: true,
      args: [
        { name: "addr", type: "string", required: true, description: "要删除的既有地址" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const config = await ipc.configGet();
  switch (action) {
    case "setMdns": {
      const saved = await ipc.configSave({ ...config, enableMdns: args.enable === true });
      markLocalWrite("config");
      return saved;
    }
    case "addBootstrap": {
      const addr = String(args.addr).trim();
      if (!isValidTransportAddr(addr)) throw new Error(`addrFormat: ${addr}`);
      if (config.bootstrap.includes(addr)) throw new Error(`addrDuplicate: ${addr}`);
      const added = await ipc.configSave({ ...config, bootstrap: [...config.bootstrap, addr] });
      markLocalWrite("config");
      return added;
    }
    case "removeBootstrap": {
      const addr = String(args.addr);
      if (!config.bootstrap.includes(addr)) {
        throw new Error(`bootstrap addr not found: ${addr}`);
      }
      const removed = await ipc.configSave({
        ...config,
        bootstrap: config.bootstrap.filter((item) => item !== addr),
      });
      markLocalWrite("config");
      return removed;
    }
    default:
      throw new Error(`discovery 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const snapshot = useNodeStore.getState();
  return {
    discovered: selectPeerList(snapshot).map((peer) => ({
      peerId: peer.peerId,
      addrs: peer.addrs,
      connected: peer.connected,
    })),
  };
}

export const discoveryPage: PageEntry = { descriptor, execute, state };
