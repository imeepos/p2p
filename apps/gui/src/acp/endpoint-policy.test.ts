import { describe, expect, it } from "vitest";

import {
  allowOnceOptionId,
  decideTier,
  emptyPolicy,
  kindBucketOf,
  rejectOptionId,
  tierCounts,
  type EndpointPolicy,
} from "./endpoint-policy";
import type { PermissionOption } from "./protocol";

const OPTIONS: PermissionOption[] = [
  { optionId: "o-allow-once", name: "Allow", kind: "allow_once" },
  { optionId: "o-allow-always", name: "Always", kind: "allow_always" },
  { optionId: "o-reject", name: "Deny", kind: "reject_once" },
];

describe("decideTier", () => {
  it("无配置全落 ask（默认档缺省不改变现行为）", () => {
    expect(decideTier(emptyPolicy(), { toolKind: "execute", title: "Run" })).toBe("ask");
    expect(decideTier(emptyPolicy(), { toolKind: null, title: "Run" })).toBe("ask");
  });

  it("类型默认档生效；未知 toolKind 落 other 桶", () => {
    const policy: EndpointPolicy = {
      defaults: { execute: "deny", other: "allow" },
      exceptions: [],
    };
    expect(decideTier(policy, { toolKind: "execute", title: "Run" })).toBe("deny");
    expect(decideTier(policy, { toolKind: "weird", title: "Run" })).toBe("allow");
    expect(decideTier(policy, { toolKind: null, title: "Run" })).toBe("allow");
  });

  it("标题精确例外优先于类型默认档；非精确匹配不命中", () => {
    const policy: EndpointPolicy = {
      defaults: { execute: "deny" },
      exceptions: [
        { title: "Run tests", tier: "allow" },
        { title: "run tests", tier: "deny" },
      ],
    };
    expect(decideTier(policy, { toolKind: "execute", title: "Run tests" })).toBe("allow");
    expect(decideTier(policy, { toolKind: "execute", title: "Run tests now" })).toBe("deny");
  });

  it("kindBucketOf：未知/缺失归 other", () => {
    expect(kindBucketOf("read")).toBe("read");
    expect(kindBucketOf(null)).toBe("other");
    expect(kindBucketOf("unknown-kind")).toBe("other");
  });

  it("tierCounts：未配置桶计 ask", () => {
    expect(tierCounts(emptyPolicy())).toEqual({ ask: 6, allow: 0, deny: 0 });
    expect(tierCounts({ defaults: { read: "allow", execute: "deny" }, exceptions: [] })).toEqual({
      ask: 4,
      allow: 1,
      deny: 1,
    });
  });
});

describe("自动应答选项选取（红线：allow 档绝不代答 allow_always）", () => {
  it("allowOnceOptionId 只取 allow_once", () => {
    expect(allowOnceOptionId(OPTIONS)).toBe("o-allow-once");
  });

  it("仅 allow_always 可选时返回 null（回落人工询问，不产生持久放行）", () => {
    expect(allowOnceOptionId([OPTIONS[1]!])).toBeNull();
  });

  it("rejectOptionId 取显式 reject 选项；无则 null（走 cancelled）", () => {
    expect(rejectOptionId(OPTIONS)).toBe("o-reject");
    expect(rejectOptionId([OPTIONS[0]!])).toBeNull();
  });
});
