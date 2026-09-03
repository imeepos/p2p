// 前端错误感知管线（契约 §8，G-H 观测）：window error / unhandledrejection / console.error 统一采集。
// Tauri 环境批量落盘 frontend.log；浏览器/mock 环境降级 localStorage——错误必须留痕，禁止只进控制台。

export type FrontendErrorKind = "error" | "unhandledrejection" | "console";

export interface FrontendErrorEntry {
  ts: string;
  kind: FrontendErrorKind;
  message: string;
  stack: string | null;
}

const MAX_BUFFER = 100;
const MESSAGE_LIMIT = 500;
const STACK_LIMIT = 2000;
const FLUSH_DEBOUNCE_MS = 400;
const LOG_STORAGE_KEY = "p2p-console.frontend-log";

let buffer: FrontendErrorEntry[] = [];
let unsent: FrontendErrorEntry[] = [];
let flushTimer: number | null = null;
let installed = false;
let originalConsoleError: ((...args: unknown[]) => void) | null = null;

export function installErrorReport(): void {
  if (installed) return;
  installed = true;
  window.addEventListener("error", (event) => {
    const err = event.error;
    record({
      kind: "error",
      message: event.message || (err ? String(err) : "unknown error"),
      stack: stackOf(err),
    });
  });
  window.addEventListener("unhandledrejection", (event) => {
    const reason = (event as PromiseRejectionEvent).reason;
    record({
      kind: "unhandledrejection",
      message: reason instanceof Error ? reason.message : stringify(reason),
      stack: stackOf(reason),
    });
  });
  originalConsoleError = console.error.bind(console);
  console.error = (...args: unknown[]) => {
    const err = args.find((a): a is Error => a instanceof Error);
    record({
      kind: "console",
      message: args.map(stringify).join(" "),
      stack: stackOf(err),
    });
    originalConsoleError?.(...args);
  };
}

export function getRecentErrors(): readonly FrontendErrorEntry[] {
  return buffer;
}

export function clearRecentErrors(): void {
  buffer = [];
}

/// 一键清理（内存侧）：清空最近错误缓冲与待发送队列，并取消防抖刷写，
/// 避免清空后残留队列又把旧错误写回持久层。
export function clearErrorBufferAndQueue(): void {
  buffer = [];
  unsent = [];
  if (flushTimer !== null) {
    window.clearTimeout(flushTimer);
    flushTimer = null;
  }
  try {
    localStorage.removeItem(LOG_STORAGE_KEY);
  } catch (err) {
    originalConsoleError?.("[error-report] localStorage 清理失败:", err);
  }
}

function record(entry: Omit<FrontendErrorEntry, "ts">): void {
  const full: FrontendErrorEntry = { ts: new Date().toISOString(), ...entry };
  buffer.push(full);
  if (buffer.length > MAX_BUFFER) buffer = buffer.slice(-MAX_BUFFER);
  unsent.push(full);
  scheduleFlush();
}

function scheduleFlush(): void {
  if (flushTimer !== null) return;
  flushTimer = window.setTimeout(() => {
    flushTimer = null;
    void flush();
  }, FLUSH_DEBOUNCE_MS);
}

async function flush(): Promise<void> {
  if (unsent.length === 0) return;
  const batch = unsent;
  unsent = [];
  if (hasTauri()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("frontend_log_append", { lines: batch.map(toJsonLine) });
    } catch (err) {
      // 失败路径必须可观测：落盘失败回滚队列并显式告警，禁止静默吞错。
      unsent.unshift(...batch);
      originalConsoleError?.("[error-report] frontend.log 写入失败:", err);
    }
  } else {
    persistLocal(batch);
  }
}

function persistLocal(batch: FrontendErrorEntry[]): void {
  try {
    const prev = localStorage.getItem(LOG_STORAGE_KEY) ?? "";
    const merged = prev + batch.map(toJsonLine).join("\n") + "\n";
    const lines = merged.split("\n").filter(Boolean);
    localStorage.setItem(LOG_STORAGE_KEY, lines.slice(-MAX_BUFFER).join("\n") + "\n");
  } catch (err) {
    originalConsoleError?.("[error-report] localStorage 降级写失败:", err);
  }
}

export function readLocalLogLines(): string[] {
  try {
    const raw = localStorage.getItem(LOG_STORAGE_KEY);
    if (!raw) return [];
    return raw.split("\n").filter(Boolean);
  } catch {
    return [];
  }
}

function hasTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function toJsonLine(entry: FrontendErrorEntry): string {
  return JSON.stringify({
    ts: entry.ts,
    kind: entry.kind,
    message: entry.message.slice(0, MESSAGE_LIMIT),
    stack: entry.stack ? entry.stack.slice(0, STACK_LIMIT) : null,
  });
}

function stackOf(err: unknown): string | null {
  return err instanceof Error && err.stack ? err.stack : null;
}

function stringify(value: unknown): string {
  if (typeof value === "string") return value;
  if (value instanceof Error) return value.message;
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}