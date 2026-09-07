import i18n from "@/i18n";
import { toastInfo } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";
import { isTauriRuntime } from "@/lib/tauri-env";

// R2-20 文档内链接治理：把 href 解析为三类落地——五篇内跨文（应用内跳转）、
// 外链（桌面端系统浏览器，mock 态降级复制+提示）、不可达仓库路径（反馈+复制）。
// 纯解析函数独立成模块，便于单测与 docs-markdown 消费。

interface DocEntry {
  id: string;
  title: string;
}

export type ResolvedDocLink =
  | { kind: "doc"; docId: string; title: string }
  | { kind: "external"; href: string }
  | { kind: "blocked"; label: string; path: string };

// 总览篇源文件是 README.md，与注册表 id protocol-overview 对不上，别名补齐
const DOC_ID_ALIASES: Record<string, string> = { readme: "protocol-overview" };

export function resolveDocLink(
  href: string | null | undefined,
  docs: readonly DocEntry[],
): ResolvedDocLink | null {
  const raw = href?.trim();
  if (!raw) return null;
  if (/^https?:\/\//i.test(raw)) return { kind: "external", href: raw };
  const base = raw.split(/[\\/]/).pop() ?? "";
  const key = base.replace(/\.md$/i, "").toLowerCase();
  const docId = DOC_ID_ALIASES[key] ?? key;
  const doc = docs.find((d) => d.id === docId);
  if (doc) return { kind: "doc", docId: doc.id, title: doc.title };
  return { kind: "blocked", label: key || raw, path: raw };
}

// 渲染文本不再裸露仓库相对路径：跨文链接用人话标题（文档 H1），不可达路径
// 用去目录去后缀的短名；title 属性保留原路径供追溯。外链锚文本不动。
export function linkDisplayLabel(
  link: ResolvedDocLink,
  anchorText: string,
): string {
  if (link.kind === "doc") return link.title;
  if (link.kind === "blocked") return link.label;
  return anchorText;
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (error) {
    console.warn("[docs] 剪贴板写入失败", error);
    return false;
  }
}

async function copyWithHint(href: string, hintKey: "docs.link.blockedCopied" | "docs.link.externalCopied"): Promise<void> {
  await copyText(href);
  toastInfo(i18n.t("docs.link.degradedTitle"), i18n.t(hintKey, { path: href }));
}

// 不可达仓库相对路径（如 ../design/wire-protocol.md）：无应用内落地页，
// 反馈 + 复制路径；console.warn 保留可观测信号不静默吞。
export async function openBlockedLink(link: Extract<ResolvedDocLink, { kind: "blocked" }>): Promise<void> {
  console.warn("[docs] 文档链接暂不支持应用内跳转: " + link.path);
  await copyWithHint(link.path, "docs.link.blockedCopied");
}

// 外链：桌面端走系统浏览器（复用 update_open_release_page 的 opener 通道，
// 该命令带 github.com 白名单，白名单外失败即降级复制+提示）；浏览器 mock 态
// 无系统浏览器，直接降级。
export async function openExternalLink(href: string): Promise<void> {
  if (!isTauriRuntime()) {
    await copyWithHint(href, "docs.link.externalCopied");
    return;
  }
  try {
    await ipc.updateOpenReleasePage(href);
  } catch (error) {
    console.error("[docs] 外链系统打开失败", href, error);
    await copyWithHint(href, "docs.link.externalCopied");
  }
}
