// 交互面纯模型：request_permission 批准态、config_option 目录、usage 占用。
// 全部为 (state, input) -> state 归并，store 只做接线（设计 §8 交互行）。
import {
  PERMISSION_TIMEOUT_MS,
  type ConfigOption,
  type PermissionOption,
  type UsageUpdate,
} from "./protocol";

export type PermissionStatus = "pending" | "approved" | "rejected";

export interface PermissionRequestView {
  /** agent 侧 JSON-RPC 请求 id，应答帧原样带回 */
  requestId: number;
  sessionId: string | null;
  title: string;
  toolKind: string | null;
  options: PermissionOption[];
  /** 收到时刻（epoch ms），60s 倒计时基准，与桥侧超时对齐 */
  receivedAt: number;
  status: PermissionStatus;
}

export interface InteractionState {
  permissions: PermissionRequestView[];
  configOptions: ConfigOption[];
  usage: UsageUpdate | null;
}

export function emptyInteraction(): InteractionState {
  return { permissions: [], configOptions: [], usage: null };
}

/** config_option_update / set_config_option 响应都是完整配置态，整表替换 */
export function applyConfigOptions(
  state: InteractionState,
  options: ConfigOption[] | undefined,
): InteractionState {
  if (!Array.isArray(options)) return state;
  return { ...state, configOptions: options.map((o) => ({ ...o })) };
}

/** usage_update：used/size 均为有限非负数才采纳，坏帧留告警不静默 */
export function applyUsage(state: InteractionState, update: UsageUpdate): InteractionState {
  const { used, size } = update;
  const ok =
    (used === undefined || (typeof used === "number" && Number.isFinite(used) && used >= 0)) &&
    (size === undefined || (typeof size === "number" && Number.isFinite(size) && size >= 0));
  if (!ok) {
    console.warn("[acp] usage_update 数值非法，已丢弃", update);
    return state;
  }
  const merged: UsageUpdate = { ...state.usage, ...update };
  return { ...state, usage: merged };
}

export function addPermission(
  state: InteractionState,
  req: Omit<PermissionRequestView, "status">,
): InteractionState {
  if (state.permissions.some((p) => p.requestId === req.requestId)) return state;
  const view: PermissionRequestView = { ...req, status: "pending" };
  return { ...state, permissions: [...state.permissions, view] };
}

/** 首个 allow_* 选项即批准目标（镜像桥侧静态策略的选取规则） */
export function allowOptionId(options: PermissionOption[]): string | null {
  const allow = options.find((o) => typeof o.kind === "string" && o.kind.startsWith("allow"));
  return allow ? allow.optionId : null;
}

export function resolvePermission(
  state: InteractionState,
  requestId: number,
  status: Exclude<PermissionStatus, "pending">,
): InteractionState {
  let changed = false;
  const permissions = state.permissions.map((p) => {
    if (p.requestId === requestId && p.status === "pending") {
      changed = true;
      return { ...p, status };
    }
    return p;
  });
  return changed ? { ...state, permissions } : state;
}

/** 断线/轮次取消：未决权限按桥约定视为已拒绝（reject-once），已决不翻转 */
export function rejectUnanswered(state: InteractionState): InteractionState {
  let changed = false;
  const permissions = state.permissions.map((p) => {
    if (p.status === "pending") {
      changed = true;
      return { ...p, status: "rejected" as const };
    }
    return p;
  });
  return changed ? { ...state, permissions } : state;
}

/** 倒计时归零判定（与桥侧 --permission-timeout-secs 默认 60s 对齐）；只需时间戳 */
export function permissionExpired(
  req: Pick<PermissionRequestView, "receivedAt">,
  now: number,
): boolean {
  return now - req.receivedAt >= PERMISSION_TIMEOUT_MS;
}

/** 剩余秒数（向上取整，供倒计时展示） */
export function permissionSecondsLeft(
  req: Pick<PermissionRequestView, "receivedAt">,
  now: number,
): number {
  return Math.max(0, Math.ceil((req.receivedAt + PERMISSION_TIMEOUT_MS - now) / 1000));
}

/** 会话清理：关闭会话时随 transcript 一并移除 */
export function dropInteraction(
  interactions: Record<string, InteractionState>,
  sessionId: string,
): Record<string, InteractionState> {
  if (!(sessionId in interactions)) return interactions;
  const next = { ...interactions };
  delete next[sessionId];
  return next;
}