/**
 * @local/dsh-acp-launch — acp profile 的自托管 stdio 启动器。
 *
 * 动机：@deepseek-ai/dsh-acp-app 要求 launcher 预先提供 ctx.appExit/appReady
 * （bounded shutdown 归 launcher 所有），而裸跑 dsh --profile acp 没有任何宿主
 * 提供这两项服务，启动即抛 "the launcher must provide ctx.appExit and
 * ctx.appReady before the tree mounts"。本模块经 cordis patch 顶替
 * acp-app-startup 条目：自备 appExit/appReady 缺省实现后复刻原插件语义
 * （零选项命令 + stdin EOF 绑定退出 + acpAppStartup 服务），不改动 harness 仓库。
 * 红线：stdout 归 ACP 协议，帮助文本与诊断只走 stderr。
 */

/** 原 acp-app-startup 插件发布的服务名（acp 桥行注入等待）。 */
const SERVICE = "acpAppStartup";

/** 保持与被顶替条目一致的插件名（loader 日志可读性）。 */
export const name = "acp-app-startup";

/** 与原插件一致：命令行快照由 launcher 提供。 */
export const inject = ["cmdlineArgs"];

const USAGE = [
  "dsh --profile acp",
  "",
  "Serve automation clients over Agent Client Protocol stdio.",
  "",
  "Example:",
  "  dsh --profile acp     serve ACP until the client disconnects",
].join("\n");

function defaultExit(code) {
  // 留一拍冲刷窗口再退出：stdin EOF 即退出是 ACP stdio 的生命周期契约。
  process.exitCode = code;
  const timer = setTimeout(() => process.exit(code), 30);
  if (typeof timer.unref === "function") timer.unref();
}

function defaultReady() {
  let fired = false;
  const pending = [];
  return {
    onReady(listener) {
      if (fired) {
        listener();
        return () => {};
      }
      pending.push(listener);
      return () => {
        const at = pending.indexOf(listener);
        if (at >= 0) pending.splice(at, 1);
      };
    },
    commit() {
      if (fired) return;
      fired = true;
      while (pending.length > 0) pending.shift()();
    },
  };
}

/**
 * 顶替 apply：补齐 launcher 服务，复刻原插件行为。
 * @param ctx - acp profile 插件上下文（cmdlineArgs 已由 dsh launcher 提供）。
 */
export function apply(ctx) {
  const args = ctx.get("cmdlineArgs")?.get?.() ?? [];
  if (args.some((a) => a === "-h" || a === "--help")) {
    process.stdout.write(USAGE + "\n");
    process.exit(0);
  }
  const hostedExit = ctx.get("appExit");
  const hostedReady = ctx.get("appReady");
  const exit = hostedExit ?? defaultExit;
  const ready = hostedReady ?? defaultReady();
  if (hostedExit === undefined) ctx.provide("appExit", exit);
  if (hostedReady === undefined) ctx.provide("appReady", ready);
  const onEnd = () => {
    // 原 semantics：EOF 后等 ready 提交再请求退出；缺省 ready 在本 apply 内即提交。
    ready.onReady(() => exit(0));
  };
  const stdin = process.stdin;
  stdin.once("end", onEnd);
  if (stdin.readableEnded) queueMicrotask(onEnd);
  if (typeof ready.commit === "function") ready.commit();
  else queueMicrotask(() => {});
  ctx.provide(SERVICE, { accepted: true });
}
