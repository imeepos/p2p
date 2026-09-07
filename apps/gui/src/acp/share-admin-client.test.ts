import { afterEach, describe, expect, it, vi } from "vitest";

import { createShare, listShares, listWorkspaces, revokeShare } from "./share-admin-client";

const ADMIN = "http://127.0.0.1:9910/";
const TOK = "admin-token";

function jsonResponse(ok: boolean, body: unknown, status = ok ? 200 : 400) {
  return {
    ok,
    status,
    json: async () => body,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("createShare POST /shares 契约（§5）", () => {
  it("Bearer 头 + JSON 体投递；响应 link 直用", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) =>
      jsonResponse(true, {
        share_id: "sid-1",
        token: "f".repeat(32),
        link: "dsh-acp-share://v1?peer=p&token=" + "f".repeat(32),
        expires_at_unix: 3_000,
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const out = await createShare(ADMIN, TOK, { scope: "sandbox", ttl_secs: 3_600 });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://127.0.0.1:9910/shares");
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer " + TOK);
    expect(JSON.parse(init.body as string)).toEqual({ scope: "sandbox", ttl_secs: 3_600 });
    expect(out.shareId).toBe("sid-1");
    expect(out.link).toContain("peer=p");
  });

  it("无 link 时按链接要素本地拼装；缺要素显式报错不静默", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        jsonResponse(true, {
          share_id: "sid-2",
          token: "e".repeat(32),
          peer: "peerA",
          addrs: ["/ip4/10.0.0.8/tcp/4001"],
        }),
      ),
    );
    const out = await createShare(ADMIN, TOK, {});
    expect(out.link).toContain("peer=peerA");
    expect(out.link).toContain("addr=%2Fip4");
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => jsonResponse(true, { share_id: "sid-3", token: "d".repeat(32) })),
    );
    await expect(createShare(ADMIN, TOK, {})).rejects.toThrow("missing link elements");
  });

  it("非 2xx（如 422 workspace 未配）抛带状态码错误供 UI 映射", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => jsonResponse(false, { error: "workspace-dir not configured" }, 422)),
    );
    await expect(createShare(ADMIN, TOK, {})).rejects.toThrow("HTTP 422");
  });
});

describe("listShares GET /shares 契约", () => {
  it("壳响应与裸数组都收；缺 share_id 条目丢弃", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        jsonResponse(true, {
          shares: [
            {
              share_id: "sid-1",
              scope: "workspace",
              max_activations: 2,
              activations: 1,
              expires_at_unix: 3_000,
              revoked: false,
              note: "n",
              created_at: "t",
              bound_peer: "peerA",
            },
            { scope: "sandbox" },
            42,
          ],
        }),
      ),
    );
    const list = await listShares(ADMIN, TOK);
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ share_id: "sid-1", scope: "workspace", bound_peer: "peerA" });
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => jsonResponse(true, [{ share_id: "sid-legacy", scope: "sandbox" }])),
    );
    expect(await listShares(ADMIN, TOK)).toHaveLength(1);
  });
});

describe("revokeShare DELETE /shares/{id} 契约", () => {
  it("DELETE 到编码后的 shareId 路径", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) => jsonResponse(true, {}));
    vi.stubGlobal("fetch", fetchMock);
    await revokeShare(ADMIN, TOK, "sid 1");
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://127.0.0.1:9910/shares/sid%201");
    expect(init.method).toBe("DELETE");
  });
});

describe("listWorkspaces GET /workspaces（多工作区加法）", () => {
  it("解析 id/name/dir 行；旧 agent 404 → 空表回落", async () => {
    const fetchMock = vi.fn(async (_url: string) =>
      jsonResponse(true, {
        workspaces: [
          { id: "ws1", name: "p2p", dir: "/home/me/p2p" },
          { id: "", name: "bad", dir: "/x" },
        ],
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const rows = await listWorkspaces(ADMIN, TOK);
    expect(rows).toEqual([{ id: "ws1", name: "p2p", dir: "/home/me/p2p" }]);
    const [url] = fetchMock.mock.calls[0] as [string];
    expect(url).toBe("http://127.0.0.1:9910/workspaces");

    vi.stubGlobal(
      "fetch",
      vi.fn(async () => jsonResponse(false, { error: "not-found" }, 404)),
    );
    expect(await listWorkspaces(ADMIN, TOK)).toEqual([]);
  });
});