// 分享链接模型（docs/design/llm-share-link-design.md §5.4 冻结格式）：解析/拼装/聊天正文识别。
// 纯逻辑层，禁止引入 React/i18n 依赖；对齐 acp/share-model.ts 的 findShareLinkInText 模式
// 但 scheme 独立（dsh-llm-share://），正则参数化避免与 ACP 固定前缀互相干扰。

export const LLM_SHARE_LINK_PREFIX = "dsh-llm-share://v1?";

/** 链接在聊天正文中的出现形态：前缀起到首个空白/引号/尖括号为止（对齐 ACP 先例） */
const LINK_IN_TEXT_RE = /dsh-llm-share:\/\/v1\?[^\s<>"']+/g;

export interface ParsedLlmShareLink {
  peer: string;
  /** 可重复出现（QUIC/TCP/中继多地址），本波 GUI 仅展示不拨号 */
  addrs: string[];
  token: string;
  /** 展示性提示；权威判定在出借方，GUI 不得因本地时钟误判而拒绝尝试 */
  expUnix: number | null;
  sid: string | null;
  /** 分享模型集（非空；借出方展示与 borrow 预填候选源） */
  models: string[];
}

export type LlmShareLinkErrorCode = "scheme" | "missing" | "token";

export class LlmShareLinkError extends Error {
  readonly code: LlmShareLinkErrorCode;
  constructor(code: LlmShareLinkErrorCode) {
    super("llm-share-link:" + code);
    this.code = code;
  }
}

/** §5.4 冻结契约：token = 128-bit CSPRNG hex（32 位十六进制） */
const TOKEN_RE = /^[0-9a-f]{32}$/;

export function parseLlmShareLink(input: string): ParsedLlmShareLink {
  const text = input.trim();
  if (!text.startsWith(LLM_SHARE_LINK_PREFIX)) throw new LlmShareLinkError("scheme");
  const params = new URLSearchParams(text.slice(LLM_SHARE_LINK_PREFIX.length));
  const peer = params.get("peer")?.trim() ?? "";
  const token = params.get("token")?.trim() ?? "";
  if (!peer || !token) throw new LlmShareLinkError("missing");
  if (!TOKEN_RE.test(token)) throw new LlmShareLinkError("token");
  const addrs = params
    .getAll("addr")
    .map((a) => a.trim())
    .filter((a) => a !== "");
  const expRaw = params.get("exp");
  const expUnix = expRaw !== null && /^\d+$/.test(expRaw) ? Number(expRaw) : null;
  const models = (params.get("models") ?? "")
    .split(",")
    .map((m) => m.trim())
    .filter((m) => m !== "");
  return { peer, addrs, token, expUnix, sid: params.get("sid"), models };
}

/** 从聊天正文中提取第一条分享链接；无则 null（渲染层识别入口）。
 *  match 而非 exec：/g 正则有 lastIndex 状态，跨调用串位是隐蔽 bug 源（ACP 先例同款） */
export function findLlmShareLinkInText(text: string): string | null {
  return text.match(LINK_IN_TEXT_RE)?.[0] ?? null;
}

export interface LlmShareLinkParts {
  peer: string;
  addrs?: string[];
  token: string;
  expUnix: number | null;
  sid: string | null;
  models: string[];
}

/** 按 §5.4 契约拼装链接（mock 出借侧生成与测试共用） */
export function buildLlmShareLink(parts: LlmShareLinkParts): string {
  const params = new URLSearchParams();
  params.set("peer", parts.peer);
  for (const addr of parts.addrs ?? []) params.append("addr", addr);
  params.set("token", parts.token);
  if (parts.expUnix !== null) params.set("exp", String(parts.expUnix));
  if (parts.sid) params.set("sid", parts.sid);
  params.set("models", parts.models.join(","));
  return LLM_SHARE_LINK_PREFIX + params.toString();
}
