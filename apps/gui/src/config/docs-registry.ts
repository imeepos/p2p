// 协议文档注册表（DOC2）：五篇 md 自仓库 docs/protocol/ 经 Vite ?raw 单源
// 引入，禁止复制进 apps/gui 造成双源漂移；目录标题取各文首个 H1（正文即
// zh-CN 单源），缺 H1 回退文档 id 并 console.warn 留观测信号。
import builtinAndVersioning from "../../../../docs/protocol/builtin-and-versioning.md?raw";
import nodeLifecycle from "../../../../docs/protocol/node-lifecycle.md?raw";
import protocolOverview from "../../../../docs/protocol/README.md?raw";
import quickstart from "../../../../docs/protocol/quickstart.md?raw";
import wireFormat from "../../../../docs/protocol/wire-format.md?raw";

export interface ProtocolDoc {
  id: string;
  title: string;
  markdown: string;
}

function titleFromH1(markdown: string, fallback: string): string {
  const heading = markdown.split("\n").find((line) => line.startsWith("# "));
  if (heading === undefined) {
    console.warn("[docs] 文档缺少 H1 标题，目录回退文档 id: " + fallback);
    return fallback;
  }
  return heading.replace(/^#\s+/, "").trim();
}

function toDoc(id: string, markdown: string): ProtocolDoc {
  return { id, title: titleFromH1(markdown, id), markdown };
}

// 阅读顺序：总览 → 快速上手 → 线格式 → 节点生命周期 → 内置协议与版本。
export const PROTOCOL_DOCS: readonly ProtocolDoc[] = [
  toDoc("protocol-overview", protocolOverview),
  toDoc("quickstart", quickstart),
  toDoc("wire-format", wireFormat),
  toDoc("node-lifecycle", nodeLifecycle),
  toDoc("builtin-and-versioning", builtinAndVersioning),
];
