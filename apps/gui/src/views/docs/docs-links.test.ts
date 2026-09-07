import { describe, expect, it } from "vitest";

import { PROTOCOL_DOCS } from "@/config/docs-registry";

import { linkDisplayLabel, resolveDocLink } from "./docs-links";

// R2-20 链接治理纯函数：跨文映射五篇、外链识别、仓库相对路径归 blocked。
const docs = PROTOCOL_DOCS;

describe("resolveDocLink", () => {
  it("五篇同名 md 映射为对应 doc（含 ./ 与子目录前缀）", () => {
    for (const name of ["quickstart", "wire-format", "node-lifecycle", "builtin-and-versioning"]) {
      const resolved = resolveDocLink(name + ".md", docs);
      expect(resolved).toEqual({
        kind: "doc",
        docId: name,
        title: docs.find((d) => d.id === name)!.title,
      });
    }
    expect(resolveDocLink("./wire-format.md", docs)?.kind).toBe("doc");
  });

  it("README.md 经别名映射到总览篇 protocol-overview", () => {
    const resolved = resolveDocLink("README.md", docs);
    expect(resolved?.kind).toBe("doc");
    expect(resolved).toMatchObject({ docId: "protocol-overview" });
  });

  it("http(s) 外链归 external", () => {
    expect(resolveDocLink("https://rfc.example/spec", docs)).toEqual({
      kind: "external",
      href: "https://rfc.example/spec",
    });
    expect(resolveDocLink("http://example.com/a.md", docs)?.kind).toBe("external");
  });

  it("不可达仓库相对路径归 blocked，label 去目录去后缀，path 保留原文", () => {
    const resolved = resolveDocLink("../design/wire-protocol.md", docs);
    expect(resolved).toEqual({
      kind: "blocked",
      label: "wire-protocol",
      path: "../design/wire-protocol.md",
    });
  });

  it("空 href 返回 null（交由调用点兜底）", () => {
    expect(resolveDocLink("", docs)).toBeNull();
    expect(resolveDocLink(undefined, docs)).toBeNull();
    expect(resolveDocLink("  ", docs)).toBeNull();
  });
});

describe("linkDisplayLabel", () => {
  it("doc 链接用人话标题（文档 H1）", () => {
    const wire = docs.find((d) => d.id === "wire-format")!;
    expect(linkDisplayLabel({ kind: "doc", docId: wire.id, title: wire.title }, "wire-format.md")).toBe(wire.title);
  });

  it("blocked 链接用短名，外链保留锚文本", () => {
    expect(linkDisplayLabel({ kind: "blocked", label: "wire-protocol", path: "../design/wire-protocol.md" }, "../design/wire-protocol.md")).toBe("wire-protocol");
    expect(linkDisplayLabel({ kind: "external", href: "https://a.b" }, "规范原文")).toBe("规范原文");
  });
});
