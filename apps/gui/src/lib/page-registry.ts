// GC3 页面语义注册表：每路由登记 descriptor{name,description,actions,state?}。
// actions 与页面按钮同源（调 store/IPC），禁止 DOM 模拟点击；危险动作
// （identity reset、移除好友类）在 descriptor 上标 confirm 并强制 args.confirm===true。
import { chatPage } from "./pages/chat-page";
import { peersPage } from "./pages/peers-page";
import { settingsPage } from "./pages/settings-page";

export const PAGE_SCHEMA_VERSION = 1;

export type PageArgType = "string" | "number" | "boolean" | "array" | "object";

export interface PageArg {
  name: string;
  type: PageArgType;
  required: boolean;
  description: string;
}

export interface PageAction {
  name: string;
  description: string;
  args: PageArg[];
  /** 危险动作标记：调用方必须传 args.confirm === true，否则 ACTION_CONFIRM_REQUIRED 拒绝 */
  confirm?: boolean;
}

export interface PageDescriptor {
  name: string;
  description: string;
  actions: PageAction[];
}

export interface PageProtocolError {
  code: string;
  message: string;
}

export interface PageEntry {
  descriptor: PageDescriptor;
  /** action 名到真实操作路径（store/IPC 同源）的执行器 */
  execute: (action: string, args: Record<string, unknown>) => Promise<unknown>;
  /** 可选：有价值的页面状态快照（如 peers 列表行） */
  state?: () => unknown;
}

export type PageActionResult =
  | { ok: true; data: unknown }
  | { ok: false; error: PageProtocolError };

/** 已注册页面（R5 示范：chat/peers/settings 全量；其余页面待 GC3b） */
export const PAGE_REGISTRY: Readonly<Record<string, PageEntry>> = {
  chat: chatPage,
  peers: peersPage,
  settings: settingsPage,
};

/** 当前页 descriptor（含可选 state 快照）；未注册页返回结构化错误 */
export function describePage(
  page: string,
): { descriptor: PageDescriptor & { state?: unknown } } | PageProtocolError {
  const entry = PAGE_REGISTRY[page];
  if (!entry) {
    return {
      code: "PAGE_NOT_REGISTERED",
      message: `页面 "${page}" 未在前端注册表登记（可用: ${Object.keys(PAGE_REGISTRY).join("/")}）`,
    };
  }
  const state = entry.state ? { state: entry.state() } : {};
  return { descriptor: { ...entry.descriptor, ...state } };
}

/** 执行页面动作：confirm 强制 → 参数校验 → 真实执行；所有拒绝路径结构化返回 */
export async function executePageAction(
  page: string,
  action: string,
  args: Record<string, unknown>,
): Promise<PageActionResult> {
  const entry = PAGE_REGISTRY[page];
  if (!entry) {
    return {
      ok: false,
      error: {
        code: "PAGE_NOT_REGISTERED",
        message: `页面 "${page}" 未在前端注册表登记`,
      },
    };
  }
  const actionDef = entry.descriptor.actions.find((a) => a.name === action);
  if (!actionDef) {
    return {
      ok: false,
      error: {
        code: "ACTION_NOT_FOUND",
        message: `页面 "${page}" 无动作 "${action}"（可用: ${entry.descriptor.actions.map((a) => a.name).join("/")}）`,
      },
    };
  }
  if (actionDef.confirm && args.confirm !== true) {
    return {
      ok: false,
      error: {
        code: "ACTION_CONFIRM_REQUIRED",
        message: `危险动作 "${action}" 必须传 confirm: true`,
      },
    };
  }
  const argError = validateArgs(actionDef.args, args);
  if (argError) return { ok: false, error: argError };
  try {
    const data = await entry.execute(action, args);
    return { ok: true, data };
  } catch (error) {
    console.error("[page-registry] 动作执行失败", page, action, error);
    return {
      ok: false,
      error: {
        code: "ACTION_FAILED",
        message: error instanceof Error ? error.message : String(error),
      },
    };
  }
}

function validateArgs(
  defs: PageArg[],
  args: Record<string, unknown>,
): PageProtocolError | null {
  for (const def of defs) {
    const value = args[def.name];
    const present = value !== undefined;
    if (def.required && !present) {
      return {
        code: "ARG_MISSING",
        message: `动作缺少必填参数 "${def.name}"（${def.description}）`,
      };
    }
    if (present && !typeMatches(value, def.type)) {
      return {
        code: "ARG_TYPE_MISMATCH",
        message: `参数 "${def.name}" 应为 ${def.type}，实际 ${typeof value}`,
      };
    }
  }
  return null;
}

function typeMatches(value: unknown, type: PageArgType): boolean {
  switch (type) {
    case "string":
      return typeof value === "string";
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "boolean":
      return typeof value === "boolean";
    case "array":
      return Array.isArray(value);
    case "object":
      return typeof value === "object" && value !== null && !Array.isArray(value);
  }
}
