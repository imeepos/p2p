// ACS1 深链兜底（纯函数便于单测）：旧 agent 深链统一落独立会话页 /agent。
// /chat?agent=X -> /agent?endpoint=X；/chat?kind=agent -> /agent。
// 其余参数原样保留（与 mergeRedirectQuery 同口径，目标参数优先），未命中返回
// null——/chat 其余形态零行为改动，页内 agent 形态拆除属 ACS2。
export function agentRedirectTarget(search: string): string | null {
  const params = new URLSearchParams(search);
  const agent = params.get("agent");
  if (agent) {
    const next = new URLSearchParams(params);
    next.delete("agent");
    next.set("endpoint", agent);
    return "/agent?" + next.toString();
  }
  if (params.get("kind") !== "agent") return null;
  const next = new URLSearchParams(params);
  next.delete("kind");
  const query = next.toString();
  return query ? "/agent?" + query : "/agent";
}
