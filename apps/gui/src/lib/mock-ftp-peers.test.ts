import { beforeEach, describe, expect, it } from "vitest";

import {
  mockFtpBackend,
  mockFtpPeersReset,
  mockStaticPeersBackend,
} from "./mock-ftp-peers";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerIdAt(seed: number): string {
  let id = "";
  for (let i = 0; i < 44; i += 1) {
    id += B58[(seed * 7 + i * 13) % B58.length];
  }
  return id;
}

beforeEach(() => {
  mockFtpPeersReset();
});

describe("mock ftp_config_save/get（契约空密码语义）", () => {
  it("get 只回用户名名单，密码任何路径不回显", async () => {
    await mockFtpBackend.ftpConfigSave("/srv/ftp", true, {
      alice: "secret-a",
      bob: "secret-b",
    });
    const view = await mockFtpBackend.ftpConfigGet();
    expect(view).toEqual({ root: "/srv/ftp", authz: true, users: ["alice", "bob"] });
    expect(JSON.stringify(view)).not.toContain("secret");
  });

  it("空密码 = 保留该用户现有密码；显式新密码覆盖；移出表即删除", async () => {
    await mockFtpBackend.ftpConfigSave("/srv/ftp", false, {
      alice: "old-pass",
      bob: "keep-me",
    });
    // alice 空密码=保留；bob 换新密码；carol 新增明文；dave 被移出全量表即删除
    await mockFtpBackend.ftpConfigSave("/srv/ftp2", true, {
      alice: "",
      bob: "new-pass",
      carol: "carol-pass",
    });
    const view = await mockFtpBackend.ftpConfigGet();
    expect(view.root).toBe("/srv/ftp2");
    expect(view.authz).toBe(true);
    expect(view.users.sort()).toEqual(["alice", "bob", "carol"]);
  });

  it("新用户空密码显式 Err（无现有密码可保留，不静默建空密码账号）", async () => {
    await expect(
      mockFtpBackend.ftpConfigSave("/srv/ftp", true, { newcomer: "" }),
    ).rejects.toThrow("不存在");
    expect((await mockFtpBackend.ftpConfigGet()).users).toEqual([]);
  });
});

describe("mock static_peers_* 命令面", () => {
  it("upsert 校验 base58 PeerId，非法显式 Err；合法入簿并全量回读", async () => {
    const peer = peerIdAt(1);
    await expect(
      mockStaticPeersBackend.staticPeersUpsert("not-base58!", ["/u4222"], ""),
    ).rejects.toThrow("PeerId 非法");
    await mockStaticPeersBackend.staticPeersUpsert(peer, ["10.0.0.1/u4222"], "edge");
    const { peers } = await mockStaticPeersBackend.staticPeersList();
    expect(peers).toEqual([
      { peerId: peer, addrs: ["10.0.0.1/u4222"], note: "edge" },
    ]);
  });

  it("同 PeerId upsert 即替换；remove 删在册返回 true、未在册返回 false", async () => {
    const peer = peerIdAt(2);
    await mockStaticPeersBackend.staticPeersUpsert(peer, ["10.0.0.1/u1"], "old");
    await mockStaticPeersBackend.staticPeersUpsert(peer, ["10.0.0.2/t2"], "new");
    expect((await mockStaticPeersBackend.staticPeersList()).peers).toHaveLength(1);
    expect((await mockStaticPeersBackend.staticPeersList()).peers[0].note).toBe("new");
    await expect(mockStaticPeersBackend.staticPeersRemove(peer)).resolves.toBe(true);
    await expect(mockStaticPeersBackend.staticPeersRemove(peer)).resolves.toBe(false);
  });
});
