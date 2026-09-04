// settings 页 descriptor：resetIdentity 与 saveAndRestart 是危险动作，
// 与 ResetIdentityDialog / use-settings-save 同源（IPC + node store），
// registry 层强制 args.confirm === true。
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "settings",
  description: "设置页：配置保存重启与身份重置",
  actions: [
    {
      name: "saveConfig",
      description: "保存配置（不重启节点），与保存栏保存按钮同源",
      args: [
        { name: "config", type: "object", required: true, description: "GuiConfig 全量对象" },
      ],
    },
    {
      name: "saveAndRestart",
      description: "保存配置并重启节点（保存 -> stop -> start，与保存栏组合按钮同源）",
      confirm: true,
      args: [
        { name: "config", type: "object", required: true, description: "GuiConfig 全量对象" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
    {
      name: "resetIdentity",
      description: "重置节点身份（生成新密钥对，与重置确认框同源）",
      confirm: true,
      args: [
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  switch (action) {
    case "saveConfig": {
      const saved = await ipc.configSave(args.config as GuiConfig);
      return { quicPort: saved.quicPort, tcpPort: saved.tcpPort };
    }
    case "saveAndRestart": {
      const config = args.config as GuiConfig;
      await ipc.configSave(config);
      const node = useNodeStore.getState();
      await node.stopNode();
      const status = await node.startNode(config);
      return { running: status.running, peerId: status.peerId };
    }
    case "resetIdentity": {
      const status = await ipc.identityReset(true);
      await useNodeStore.getState().refresh();
      return { peerId: status.peerId };
    }
    default:
      throw new Error(`settings 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const status = useNodeStore.getState().status;
  return {
    running: status?.running ?? null,
    peerId: status?.peerId ?? null,
  };
}

export const settingsPage: PageEntry = { descriptor, execute, state };
