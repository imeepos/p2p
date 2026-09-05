// endpoint 权限策略纯模型（app-shell-redesign §3.3 第 3 块）：按操作类型
// 默认档 + 按请求标题精确匹配的例外规则；变更只影响后续到达的权限请求。
// 自动应答红线：allow 档只逐次放行（allow_once），绝不代答 allow_always，
// 不产生持久授权（acp-over-p2p-design §6 授权红线）。
import type { PermissionOption } from "./protocol";

export type PolicyTier = "ask" | "allow" | "deny";

export interface PolicyException {
  /** 请求标题原文精确匹配 */
  title: string;
  tier: PolicyTier;
}

export interface EndpointPolicy {
  /** toolKind -> 默认档；未登记的类型落 other 桶，桶也未登记时为 ask */
  defaults: Record<string, PolicyTier>;
  exceptions: PolicyException[];
}

/** 策略编辑面提供的类型桶（unknown kind 归 other） */
export const POLICY_KIND_BUCKETS = ["read", "write", "execute", "fetch", "think", "other"] as const;

export type PolicyKindBucket = (typeof POLICY_KIND_BUCKETS)[number];

/** toolKind -> 展示桶：未知/缺失归 other */
export function kindBucketOf(toolKind: string | null): PolicyKindBucket {
  if (toolKind && (POLICY_KIND_BUCKETS as readonly string[]).includes(toolKind)) {
    return toolKind as PolicyKindBucket;
  }
  return "other";
}

export function emptyPolicy(): EndpointPolicy {
  return { defaults: {}, exceptions: [] };
}

function bucketTier(policy: EndpointPolicy, bucket: PolicyKindBucket): PolicyTier | undefined {
  return policy.defaults[bucket];
}

/** 决策顺序：标题精确例外 -> 类型默认档 -> ask */
export function decideTier(
  policy: EndpointPolicy,
  req: { toolKind: string | null; title: string },
): PolicyTier {
  const hit = policy.exceptions.find((e) => e.title === req.title);
  if (hit) return hit.tier;
  const bucket = kindBucketOf(req.toolKind);
  return bucketTier(policy, bucket) ?? "ask";
}

/** 行内权限档摘要（§3.1 agent 行）：六桶按档位计数 */
export function tierCounts(policy: EndpointPolicy): Record<PolicyTier, number> {
  const counts: Record<PolicyTier, number> = { ask: 0, allow: 0, deny: 0 };
  for (const bucket of POLICY_KIND_BUCKETS) {
    counts[bucketTier(policy, bucket) ?? "ask"] += 1;
  }
  return counts;
}

/** allow 档应答目标：仅 allow_once（无 once 选项则返回 null 回落人工询问） */
export function allowOnceOptionId(options: PermissionOption[]): string | null {
  const once = options.find(
    (o) =>
      typeof o.kind === "string" &&
      o.kind.toLowerCase().startsWith("allow") &&
      !o.kind.toLowerCase().includes("always"),
  );
  return once ? once.optionId : null;
}

/** deny 档应答目标：优先显式 reject_* 选项；无则 null（走 cancelled 代答） */
export function rejectOptionId(options: PermissionOption[]): string | null {
  const reject = options.find(
    (o) => typeof o.kind === "string" && o.kind.toLowerCase().startsWith("reject"),
  );
  return reject ? reject.optionId : null;
}
