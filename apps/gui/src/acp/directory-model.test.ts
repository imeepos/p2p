// 连接目录纯模型单测：发现上迁、手动添加幂等、scope 迁移与移除。
import { describe, expect, it } from "vitest";

import {
  addManual,
  groupBySource,
  removeEntry,
  setEntryScope,
  upsertDiscovered,
  type DirectoryEntry,
} from "./directory-model";

describe("directory model", () => {
  it("addManual 建 sandbox/manual 条目；空 peer 与重复添加幂等", () => {
    let entries: DirectoryEntry[] = [];
    entries = addManual(entries, " peer-x ");
    expect(entries).toEqual([
      { peer: "peer-x", name: null, scope: "sandbox", source: "manual", addrs: [], via: null },
    ]);
    expect(addManual(entries, "")).toBe(entries);
    expect(addManual(entries, "peer-x")).toBe(entries);
  });

  it("upsertDiscovered 新节点默认 sandbox/discovered；已有条目只补 addrs 不降级", () => {
    let entries = addManual([], "peer-x");
    entries = setEntryScope(entries, "peer-x", "owner");
    entries = upsertDiscovered(entries, [
      { peer: "peer-x", addrs: ["/ip4/10.0.0.8/tcp/4001"] },
      { peer: "peer-d", addrs: ["/ip4/10.0.0.9/tcp/4001"] },
    ]);
    expect(entries.find((e) => e.peer === "peer-x")).toMatchObject({
      scope: "owner",
      source: "manual",
      addrs: ["/ip4/10.0.0.8/tcp/4001"],
    });
    expect(entries.find((e) => e.peer === "peer-d")).toMatchObject({
      scope: "sandbox",
      source: "discovered",
    });
  });

  it("setEntryScope 与 removeEntry 按 peer 精确生效", () => {
    let entries = upsertDiscovered([], [{ peer: "a" }, { peer: "b" }]);
    entries = setEntryScope(entries, "a", "workspace");
    expect(entries.find((e) => e.peer === "a")?.scope).toBe("workspace");
    entries = removeEntry(entries, "a");
    expect(entries.map((e) => e.peer)).toEqual(["b"]);
  });

  it("groupBySource 按来源分两组且组内保持原顺序", () => {
    const entries = upsertDiscovered([], [{ peer: "d-1" }, { peer: "d-2" }]);
    const mixed = addManual(entries, "m-1");
    const groups = groupBySource(mixed);
    expect(groups.discovered.map((e) => e.peer)).toEqual(["d-1", "d-2"]);
    expect(groups.manual.map((e) => e.peer)).toEqual(["m-1"]);
  });

  it("/discovery 响应映射：owner 声明名/来源/地址进发现组（目标二）", () => {
    const entries = upsertDiscovered(
      [],
      [
        {
          peer: "peer-mdns",
          addrs: ["/ip4/10.0.0.8/tcp/4001"],
          name: "home-agent",
          source: "mdns",
        },
      ],
    );
    expect(entries).toMatchObject([
      {
        peer: "peer-mdns",
        name: "home-agent",
        via: "mdns",
        addrs: ["/ip4/10.0.0.8/tcp/4001"],
        source: "discovered",
        scope: "sandbox",
      },
    ]);
  });

  it("缺字段容错：name/source/addrs 缺失或非法不炸且如实留空（目标二）", () => {
    const entries = upsertDiscovered([], [
      { peer: "peer-bare" },
      { peer: "peer-bad-addrs", addrs: "not-an-array" as unknown as string[] },
      { peer: "peer-null-name", name: null, source: null },
      { peer: 42 as unknown as string, addrs: [] },
      null,
    ] as never);
    expect(entries.map((e) => e.peer)).toEqual(["peer-bare", "peer-bad-addrs", "peer-null-name"]);
    const bare = entries.find((e) => e.peer === "peer-bare");
    expect(bare).toMatchObject({ name: null, via: null, addrs: [] });
  });

  it("去重合并：同 peer 跨快照合并地址不重复（目标二）", () => {
    let entries = upsertDiscovered([], [
      { peer: "peer-x", addrs: ["/ip4/10.0.0.8/tcp/4001"], source: "mdns" },
    ]);
    entries = upsertDiscovered(entries, [
      {
        peer: "peer-x",
        addrs: ["/ip4/10.0.0.8/tcp/4001", "/ip4/10.0.0.9/tcp/4001"],
        name: "renamed",
        source: "rendezvous",
      },
    ]);
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({
      addrs: ["/ip4/10.0.0.8/tcp/4001", "/ip4/10.0.0.9/tcp/4001"],
      name: "renamed",
      via: "rendezvous",
    });
  });
});