// 分享链接模型（docs/design/acp-share-design.md §2/§3/§8）：链接解析、
// 台账状态推导、创建表单校验。纯逻辑层，禁止引入 React/i18n 依赖。
// 链接格式为冻结契约：scheme 不符即拒绝；peer/token 缺失即拒绝；未知参数忽略。

export type ShareScope = "sandbox" | "workspace";

export const SHARE_LINK_PREFIX = "dsh-acp-share://v1?";

/** 链接在聊天正文中的出现形态：前缀起到首个空白/引号/尖括号为止 */
const LINK_IN_TEXT_RE = /dsh-acp-share:\/\/v1\?[^\s<>"']+/g;

export interface ParsedShareLink {
  peer: string;
  /** 可重复出现（QUIC/TCP/中继多地址），guest 侧逐个登记候选 */
  addrs: string[];
  token: string;
  /** 展示性提示；权威判定在 agent 侧，GUI 不得因本地时钟误判而拒绝尝试 */
  expUnix: number | null;
  sid: string | null;
}

export type ShareLinkErrorCode = "scheme" | "missing" | "token";

export class ShareLinkError extends Error {
  readonly code: ShareLinkErrorCode;
  constructor(code: ShareLinkErrorCode) {
    super("share-link:" + code);
    this.code = code;
  }
}

/** §2 冻结契约：token = 128-bit 随机 hex */
const TOKEN_RE = /^[0-9a-f]{32}$/;

export function parseShareLink(input: string): ParsedShareLink {
  const text = input.trim();
  if (!text.startsWith(SHARE_LINK_PREFIX)) throw new ShareLinkError("scheme");
  const params = new URLSearchParams(text.slice(SHARE_LINK_PREFIX.length));
  const peer = params.get("peer")?.trim() ?? "";
  const token = params.get("token")?.trim() ?? "";
  if (!peer || !token) throw new ShareLinkError("missing");
  if (!TOKEN_RE.test(token)) throw new ShareLinkError("token");
  const addrs = params
    .getAll("addr")
    .map((a) => a.trim())
    .filter((a) => a !== "");
  const expRaw = params.get("exp");
  const expUnix = expRaw !== null && /^\d+$/.test(expRaw) ? Number(expRaw) : null;
  return { peer, addrs, token, expUnix, sid: params.get("sid") };
}

/** 从聊天正文中提取第一条分享链接；无则 null（渲染层识别入口）。
 *  match 而非 exec：/g 正则有 lastIndex 状态，跨调用串位是隐蔽 bug 源 */
export function findShareLinkInText(text: string): string | null {
  return text.match(LINK_IN_TEXT_RE)?.[0] ?? null;
}

/** agent 台账条目（§3 ShareEntry 的 GUI 视角形状，GET /shares 脱敏面） */
export interface ShareEntry {
  share_id: string;
  scope: ShareScope;
  /** 定向工作区 id（多工作区加法；null/缺省 = 默认工作区） */
  workspace?: string | null;
  allow_mcp: string[];
  max_activations: number;
  activations: number;
  expires_at_unix: number;
  revoked: boolean;
  note: string;
  created_at: string;
  bound_peer: string | null;
}

export type ShareStatus = "active" | "bound" | "exhausted" | "expired" | "revoked";

/** 状态徽章推导（§8 五态）：撤销 > 过期 > 用尽 > 绑定 > 有效（单徽章优先级） */
export function shareStatus(entry: ShareEntry, nowUnix: number): ShareStatus {
  if (entry.revoked) return "revoked";
  if (entry.expires_at_unix <= nowUnix) return "expired";
  if (entry.activations >= entry.max_activations) return "exhausted";
  if (entry.bound_peer) return "bound";
  return "active";
}

export const SHARE_TTL_OPTIONS = [
  { key: "1h", secs: 3_600 },
  { key: "24h", secs: 86_400 },
  { key: "7d", secs: 604_800 },
] as const;

export type ShareTtlKey = (typeof SHARE_TTL_OPTIONS)[number]["key"];

export function ttlSecs(key: ShareTtlKey): number {
  return SHARE_TTL_OPTIONS.find((o) => o.key === key)?.secs ?? 86_400;
}

export interface ShareCreateInput {
  scope: ShareScope;
  /** scope=workspace 时定向的工作区 id；缺省 = agent 默认工作区 */
  workspaceId?: string | null;
  ttl: ShareTtlKey;
  maxActivations: number;
  note: string;
}

export interface ShareCreateErrors {
  activations?: boolean;
  note?: boolean;
}

const NOTE_MAX_CHARS = 200;
export const ACTIVATIONS_MAX = 99;

/** 弹层表单校验：激活次数为 1-99 整数、备注限长；其余字段由控件约束 */
export function validateShareCreate(input: {
  maxActivations: number;
  note: string;
}): ShareCreateErrors {
  const errors: ShareCreateErrors = {};
  if (
    !Number.isInteger(input.maxActivations) ||
    input.maxActivations < 1 ||
    input.maxActivations > ACTIVATIONS_MAX
  ) {
    errors.activations = true;
  }
  if (input.note.trim().length > NOTE_MAX_CHARS) errors.note = true;
  return errors;
}

export function hasShareCreateErrors(errors: ShareCreateErrors): boolean {
  return errors.activations === true || errors.note === true;
}

/** POST /shares 请求体（§5 契约：scope/ttl_secs 必填，其余字段显式带默认） */
export function shareCreateBody(input: ShareCreateInput): Record<string, unknown> {
  const body: Record<string, unknown> = {
    scope: input.scope,
    ttl_secs: ttlSecs(input.ttl),
    max_activations: input.maxActivations,
    note: input.note.trim(),
  };
  if (input.workspaceId) body.workspace = input.workspaceId;
  return body;
}

export interface ShareLinkParts {
  peer: string;
  addrs: string[];
  token: string;
  expUnix: number | null;
  sid: string | null;
}

/** agent 未回传组装好的 link 时按 §2 契约本地拼装（首地址入链） */
export function buildShareLink(parts: ShareLinkParts): string {
  const params = new URLSearchParams();
  params.set("peer", parts.peer);
  for (const addr of parts.addrs) params.append("addr", addr);
  params.set("token", parts.token);
  if (parts.expUnix !== null) params.set("exp", String(parts.expUnix));
  if (parts.sid) params.set("sid", parts.sid);
  return SHARE_LINK_PREFIX + params.toString();
}