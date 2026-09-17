import type {
  FtpConfigView,
  StaticPeerView,
  StaticPeersList,
} from "./ipc-types";

// W2b 契约 mock（ftp_config_* / static_peers_*）：内存态独立文件，沿 mock-services
// 先例由 mock-ipc 装配。密码语义与真后端一致：get 只回用户名名单不回显；
// save 的 accounts 为全量目标表，空密码 = 保留该用户现有密码。
const B58_RE = /^[1-9A-HJ-NP-Za-km-z]{43,45}$/;

const ftp: { root: string; authz: boolean; accounts: Map<string, string> } = {
  root: "",
  authz: false,
  accounts: new Map(),
};

const staticPeers = new Map<string, StaticPeerView>();

export const mockFtpBackend = {
  async ftpConfigGet(): Promise<FtpConfigView> {
    return { root: ftp.root, authz: ftp.authz, users: [...ftp.accounts.keys()] };
  },

  async ftpConfigSave(
    root: string,
    authz: boolean,
    accounts: Record<string, string>,
  ): Promise<boolean> {
    const next = new Map<string, string>();
    for (const [user, password] of Object.entries(accounts)) {
      if (password === "") {
        const kept = ftp.accounts.get(user);
        // 契约里空密码的唯一含义是「保留现有密码」；新用户无处可保留，
        // 镜像后端显式 Err，不静默创建空密码账号。
        if (kept === undefined) {
          throw new Error(`用户 ${user} 不存在，空密码仅用于保留现有密码`);
        }
        next.set(user, kept);
      } else {
        next.set(user, password);
      }
    }
    ftp.root = root;
    ftp.authz = authz;
    ftp.accounts = next;
    return true;
  },
};

export const mockStaticPeersBackend = {
  async staticPeersList(): Promise<StaticPeersList> {
    return {
      peers: [...staticPeers.values()].map((peer) => ({
        ...peer,
        addrs: [...peer.addrs],
      })),
    };
  },

  async staticPeersUpsert(
    peerId: string,
    addrs: string[],
    note: string,
  ): Promise<boolean> {
    if (!B58_RE.test(peerId)) {
      throw new Error(`PeerId 非法（需 base58，43-45 字符）：${peerId}`);
    }
    staticPeers.set(peerId, { peerId, addrs: [...addrs], note });
    return true;
  },

  async staticPeersRemove(peerId: string): Promise<boolean> {
    return staticPeers.delete(peerId);
  },
};

// 测试夹具复位：真实后端此二面为持久化配置，不随 node_stop/identity_reset 清空。
export function mockFtpPeersReset(): void {
  ftp.root = "";
  ftp.authz = false;
  ftp.accounts.clear();
  staticPeers.clear();
}
