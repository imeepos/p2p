import { afterEach, describe, expect, it, vi } from "vitest";

import {
  buildShareLink,
  findShareLinkInText,
  parseShareLink,
  ShareLinkError,
  shareCreateBody,
  shareStatus,
  SHARE_TTL_OPTIONS,
  ttlSecs,
  validateShareCreate,
  type ShareEntry,
} from "./share-model";

const TOKEN = "a".repeat(32);

function entry(patch: Partial<ShareEntry>): ShareEntry {
  return {
    share_id: "sid-1",
    scope: "sandbox",
    allow_mcp: [],
    max_activations: 1,
    activations: 0,
    expires_at_unix: 3_000,
    revoked: false,
    note: "",
    created_at: "2026-09-06T00:00:00Z",
    bound_peer: null,
    ...patch,
  };
}

describe("parseShareLink §2 冻结契约", () => {
  it("全参数解析：peer/addr 可重复/token/exp/sid，未知参数忽略", () => {
    const link =
      "dsh-acp-share://v1?peer=peerA&addr=/ip4/10.0.0.8/udp/4001/quic-v1" +
      "&addr=/ip4/10.0.0.8/tcp/4001&token=" + TOKEN + "&exp=3000&sid=sid-1&future=x";
    const parsed = parseShareLink(link);
    expect(parsed.peer).toBe("peerA");
    expect(parsed.addrs).toEqual([
      "/ip4/10.0.0.8/udp/4001/quic-v1",
      "/ip4/10.0.0.8/tcp/4001",
    ]);
    expect(parsed.token).toBe(TOKEN);
    expect(parsed.expUnix).toBe(3000);
    expect(parsed.sid).toBe("sid-1");
  });

  it("scheme 不符拒绝；peer/token 缺失拒绝；token 非 32hex 拒绝", () => {
    expect(() => parseShareLink("https://example.com?peer=a&token=b")).toThrow(ShareLinkError);
    expect(() => parseShareLink("dsh-acp-share://v2?peer=a")).toThrow(ShareLinkError);
    expect(() => parseShareLink("dsh-acp-share://v1?token=" + TOKEN)).toThrow(
      new ShareLinkError("missing"),
    );
    expect(() => parseShareLink("dsh-acp-share://v1?peer=a")).toThrow(
      new ShareLinkError("missing"),
    );
    expect(() => parseShareLink("dsh-acp-share://v1?peer=a&token=xyz")).toThrow(
      new ShareLinkError("token"),
    );
  });

  it("exp/sid 展示性字段可缺省；坏 exp 不误判为合法时刻", () => {
    const parsed = parseShareLink("dsh-acp-share://v1?peer=a&token=" + TOKEN);
    expect(parsed.expUnix).toBeNull();
    expect(parsed.sid).toBeNull();
    const bad = parseShareLink("dsh-acp-share://v1?peer=a&token=" + TOKEN + "&exp=soon");
    expect(bad.expUnix).toBeNull();
  });
});

describe("findShareLinkInText 渲染层识别", () => {
  it("正文独占链接与嵌在句中链接都可提取；无链接返回 null", () => {
    const link = "dsh-acp-share://v1?peer=a&token=" + TOKEN;
    expect(findShareLinkInText(link)).toBe(link);
    expect(findShareLinkInText("来，用这个加入：" + link + " 过期前记得用")).toBe(link);
    expect(findShareLinkInText("普通消息没有链接")).toBeNull();
  });
});

describe("shareStatus 五态推导（单徽章优先级）", () => {
  const NOW = 1_000;
  it("撤销 > 过期 > 用尽 > 绑定 > 有效", () => {
    expect(shareStatus(entry({ revoked: true }), NOW)).toBe("revoked");
    expect(shareStatus(entry({ expires_at_unix: 1_000 }), NOW)).toBe("expired");
    expect(shareStatus(entry({ activations: 1 }), NOW)).toBe("exhausted");
    expect(shareStatus(entry({ bound_peer: "peerA", max_activations: 2 }), NOW)).toBe("bound");
    expect(shareStatus(entry({}), NOW)).toBe("active");
  });
});

describe("创建表单校验与请求体映射", () => {
  it("激活次数须为 1-99 整数；备注限 200 字", () => {
    expect(validateShareCreate({ maxActivations: 1, note: "" })).toEqual({});
    expect(validateShareCreate({ maxActivations: 0, note: "" }).activations).toBe(true);
    expect(validateShareCreate({ maxActivations: 1.5, note: "" }).activations).toBe(true);
    expect(validateShareCreate({ maxActivations: 100, note: "" }).activations).toBe(true);
    expect(validateShareCreate({ maxActivations: 1, note: "x".repeat(201) }).note).toBe(true);
  });

  it("请求体按 §5 契约映射 ttl 档位并裁剪备注", () => {
    for (const opt of SHARE_TTL_OPTIONS) {
      expect(ttlSecs(opt.key)).toBe(opt.secs);
    }
    expect(
      shareCreateBody({ scope: "sandbox", ttl: "24h", maxActivations: 3, note: "  给小明  " }),
    ).toEqual({ scope: "sandbox", ttl_secs: 86_400, max_activations: 3, note: "给小明" });
  });
});

describe("buildShareLink 本地拼装兜底", () => {
  it("按 §2 顺序拼参数，多地址重复出现", () => {
    const link = buildShareLink({
      peer: "peerA",
      addrs: ["/ip4/10.0.0.8/tcp/4001", "/ip4/10.0.0.9/tcp/4001"],
      token: TOKEN,
      expUnix: 3_000,
      sid: "sid-1",
    });
    const round = parseShareLink(link);
    expect(round.peer).toBe("peerA");
    expect(round.addrs).toHaveLength(2);
    expect(round.token).toBe(TOKEN);
  });
});
