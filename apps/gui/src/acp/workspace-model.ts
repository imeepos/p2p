// 工作区→会话两级分组的纯函数层（uix-spec §4.2）：store 只存 UI 态不冗余存树，
// 树在组件层用 selector 派生。本仓无 workspace 实体，组键=规范化 cwd（零后端改动）。
import type { SessionSummary } from "./protocol";

export interface WorkspaceGroupNode {
  /** 规范化 cwd；无 cwd → ""（未分组，恒排最后，uix-spec I1） */
  key: string;
  /** basename(cwd)；未分组为空串，渲染层用 i18n 文案兜底 */
  label: string;
  cwd: string | null;
  sessions: SessionSummary[];
  /** 组内含当前选中会话（folder 高亮 + 强制展开的依据，uix-spec I2/I3） */
  containsCurrent: boolean;
}

export const UNGROUPED_KEY = "";

/** 折叠态每组行内限流条数（uix-spec I4，参考实现 COLLAPSED_SESSION_LIMIT） */
export const COLLAPSED_SESSION_LIMIT = 5;

/** 组行 testid 后缀：未分组键为空串，用固定字面量兜底，避免选择器悬空 */
export function sessionGroupTestId(key: string): string {
  return key || "ungrouped";
}

/** cwd 规范化：trim + 去尾部斜杠；空/缺失回 null（落未分组）。根路径 "/" 原样保留 */
export function normalizeCwd(cwd: string | null | undefined): string | null {
  const trimmed = cwd?.trim() ?? "";
  if (!trimmed) return null;
  const stripped = trimmed.replace(/\/+$/, "");
  return stripped.length > 0 ? stripped : trimmed;
}

export function groupKeyOf(cwd: string | null | undefined): string {
  return normalizeCwd(cwd) ?? UNGROUPED_KEY;
}

/** 组名=basename(cwd)（tree.ts workspaceLabel 同规则）；根目录取整串 */
export function workspaceLabelOf(cwd: string | null | undefined): string {
  const normalized = normalizeCwd(cwd);
  if (!normalized) return "";
  const segments = normalized.split("/");
  return segments[segments.length - 1] || normalized;
}

/** 组序=cwd 首次出现序（列表序稳定），未分组恒末位 */
export function groupSessionsByWorkspace(
  sessions: readonly SessionSummary[],
  currentSessionId: string | null,
): WorkspaceGroupNode[] {
  const byKey = new Map<string, WorkspaceGroupNode>();
  for (const session of sessions) {
    const key = groupKeyOf(session.cwd);
    let group = byKey.get(key);
    if (!group) {
      const normalized = normalizeCwd(session.cwd);
      group = {
        key,
        label: normalized ? workspaceLabelOf(normalized) : "",
        cwd: normalized,
        sessions: [],
        containsCurrent: false,
      };
      byKey.set(key, group);
    }
    group.sessions.push(session);
    if (session.sessionId === currentSessionId) group.containsCurrent = true;
  }
  const all = [...byKey.values()];
  return [...all.filter((g) => g.key !== UNGROUPED_KEY), ...all.filter((g) => g.key === UNGROUPED_KEY)];
}

/** 组开合派生：含当前会话强制展开（uix-spec I3），其余看用户折叠清单 */
export function isGroupOpen(
  key: string,
  containsCurrent: boolean,
  collapsedKeys: readonly string[],
): boolean {
  if (containsCurrent) return true;
  return !collapsedKeys.includes(key);
}

/** 行内限流：折叠态每组只显 limit 条，超出交由「展开 N 个」按钮（uix-spec I4） */
export function collapseGroupRows(
  sessions: readonly SessionSummary[],
  opts: { limit: number },
): { rows: SessionSummary[]; hiddenCount: number } {
  const limit = Math.max(0, opts.limit);
  if (sessions.length <= limit) return { rows: [...sessions], hiddenCount: 0 };
  return { rows: sessions.slice(0, limit), hiddenCount: sessions.length - limit };
}

/** 本地过滤（uix-spec I8，砍远端内容搜索）：匹配标题/会话 id/所属组名，trim+lowercase */
export function filterSessionsByQuery(
  sessions: readonly SessionSummary[],
  query: string,
): SessionSummary[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [...sessions];
  return sessions.filter((session) => {
    const title = (session.title ?? "").toLowerCase();
    const id = session.sessionId.toLowerCase();
    const label = workspaceLabelOf(session.cwd).toLowerCase();
    return title.includes(needle) || id.includes(needle) || label.includes(needle);
  });
}
