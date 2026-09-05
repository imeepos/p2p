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
      { peer: "peer-x", name: null, scope: "sandbox", source: "manual", addrs: [] },
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
});