// 数据文件实时感知（W 波 W1）：后端 watcher → data-changed{domains} → 本模块
// 单监听器按域分发到注册的定向 reloader（禁全应用重载）。
// 回声抑制：GUI 自身写盘也会触发 data-changed，markLocalWrite 记录写序号时间，
// 窗口内的同域事件视为回声跳过（store 已在写响应中同步过，无需重载）。

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

export type DataDomain = "config" | "profile" | "chat";

export const DATA_DOMAINS: readonly DataDomain[] = ["config", "profile", "chat"];

// 抑制窗口须覆盖「本地写 → 原子落盘 → 后端防抖(≥500ms) → 事件回流」全链路。
export const ECHO_SUPPRESS_MS = 2000;

interface DataChangedPayload {
  domains?: string[];
}

interface WatchStatusPayload {
  active?: boolean;
  reason?: string | null;
}

export interface DataWatchState {
  /** R3 降级可判：监听安装失败或后端显式降级时为 true。 */
  degraded: boolean;
  reason: string | null;
  appliedCount: number;
  suppressedCount: number;
  lastAppliedAt: number | null;
}

export const useDataWatchStore = create<DataWatchState>()(() => ({
  degraded: false,
  reason: null,
  appliedCount: 0,
  suppressedCount: 0,
  lastAppliedAt: null,
}));

type Reloader = () => void;

const reloaders = new Map<DataDomain, Set<Reloader>>();
const lastLocalWrite = new Map<DataDomain, number>();

let started = false;
let nowFn = () => Date.now();

/** 测试注入时钟（回声抑制窗口判定）。 */
export function setNowFnForTest(fn: () => number): void {
  nowFn = fn;
}

/** 测试专用：清空单例状态（started/注册表/抑制表/降级态）。 */
export function resetForTest(): void {
  started = false;
  reloaders.clear();
  lastLocalWrite.clear();
  nowFn = () => Date.now();
  useDataWatchStore.setState({
    degraded: false,
    reason: null,
    appliedCount: 0,
    suppressedCount: 0,
    lastAppliedAt: null,
  });
}

/** 所有权标记：GUI 本域写盘成功后调用，窗口内的 data-changed 视为自身回声。 */
export function markLocalWrite(domain: DataDomain): void {
  lastLocalWrite.set(domain, nowFn());
}

/** 注册某域的定向重载器（store.load / hook reload）；返回注销函数。 */
export function registerReloader(domain: DataDomain, reloader: Reloader): () => void {
  let set = reloaders.get(domain);
  if (!set) {
    set = new Set();
    reloaders.set(domain, set);
  }
  set.add(reloader);
  return () => {
    set.delete(reloader);
  };
}

function isEcho(domain: DataDomain): boolean {
  const at = lastLocalWrite.get(domain);
  return at !== undefined && nowFn() - at < ECHO_SUPPRESS_MS;
}

function isDataDomain(raw: string): raw is DataDomain {
  return DATA_DOMAINS.includes(raw as DataDomain);
}

/** 按域分发：抑制域计数并留 console 信号，生效域执行 reloader 并落感知日志。 */
export function dispatchDataChanged(domains: string[]): void {
  const applied: DataDomain[] = [];
  const suppressed: DataDomain[] = [];
  for (const raw of domains) {
    if (!isDataDomain(raw)) continue;
    if (isEcho(raw)) {
      suppressed.push(raw);
      continue;
    }
    applied.push(raw);
    for (const reloader of reloaders.get(raw) ?? []) reloader();
  }
  if (suppressed.length > 0) {
    useDataWatchStore.setState((s) => ({
      suppressedCount: s.suppressedCount + suppressed.length,
    }));
    console.info("[data-watch] 自身写回声抑制", suppressed);
  }
  if (applied.length > 0) {
    useDataWatchStore.setState((s) => ({
      appliedCount: s.appliedCount + applied.length,
      lastAppliedAt: nowFn(),
    }));
    void logPerception(applied);
  }
}

// 感知证据落 frontend.log（E2E/排障直接读文件）：外部写入 → GUI 已感知的
// 可观测链路终点；失败仅 console 告警（日志是辅助面，不倒挂主链路）。
async function logPerception(domains: DataDomain[]): Promise<void> {
  try {
    const line = JSON.stringify({ kind: "data-changed", domains, ts: nowFn() });
    await invoke("frontend_log_append", { lines: [line] });
  } catch (error) {
    console.warn("[data-watch] 感知日志落盘失败", error);
  }
}

// 感知链路就绪标记（E2E 轮询门）：监听装好才会写，先于任何 data-changed 证据行。
async function logReady(): Promise<void> {
  try {
    const line = JSON.stringify({ kind: "data-watch-ready", ts: nowFn() });
    await invoke("frontend_log_append", { lines: [line] });
  } catch (error) {
    console.warn("[data-watch] 就绪标记落盘失败", error);
  }
}

// 通路探针：先于 listen 落盘，区分「webview/invoke 未通」与「listen 未通」。
async function logBoot(): Promise<void> {
  try {
    const line = JSON.stringify({ kind: "data-watch-boot", ts: nowFn() });
    await invoke("frontend_log_append", { lines: [line] });
  } catch (error) {
    console.error("[data-watch] boot 标记落盘失败", error);
  }
}

/** 单监听器安装（幂等单例）：data-changed 分发 + data-watch-status 降级可判。 */
export async function startDataWatch(): Promise<void> {
  if (started) return;
  started = true;
  void logBoot();
  try {
    const unChanged = await listen<DataChangedPayload>("data-changed", (event) => {
      dispatchDataChanged(Array.isArray(event.payload.domains) ? event.payload.domains : []);
    });
    const unStatus = await listen<WatchStatusPayload>("data-watch-status", (event) => {
      useDataWatchStore.setState({
        degraded: event.payload.active === false,
        reason: event.payload.reason ?? null,
      });
    });
    void unChanged;
    void unStatus;
    void logReady();
  } catch (error) {
    // 失败留信号：降级态（诊断面可判）+ console；不重试避免重复订阅。
    const reason = error instanceof Error ? error.message : String(error);
    useDataWatchStore.setState({ degraded: true, reason });
    console.error("[data-watch] 监听安装失败（降级：外部写入需手动刷新）", error);
  }
}
