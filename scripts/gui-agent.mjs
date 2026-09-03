#!/usr/bin/env node
// GUI Agent 操作入口（G-H 观测）：零依赖 CDP 客户端，驱动 vite dev 页面（默认 mock IPC）。
// 前置：pnpm -C apps/gui dev（端口 5173）；本机 Chrome（环境变量 CHROME_BIN 可覆盖）。
// 用法：
//   node scripts/gui-agent.mjs snap   [out.png]    全页截图（默认 .gui-agent/last.png）
//   node scripts/gui-agent.mjs errors              控制台+异常+应用内错误缓冲（JSON）
//   node scripts/gui-agent.mjs eval   "<js>"       页面内任意 JS（awaitPromise，打印结果）
//   node scripts/gui-agent.mjs click "<selector>"  querySelector(...).click()
// 通用参数：--url http://localhost:5173

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const DEBUG_PORT = 9223;
const SETTLE_MS = 1200;
const LOAD_TIMEOUT_MS = 15000;
const CHROME_CANDIDATES = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
].filter(Boolean);

function parseArgs(argv) {
  const urlFlag = argv.indexOf("--url");
  const url = urlFlag >= 0 ? argv[urlFlag + 1] : "http://localhost:5173";
  const rest = argv.filter((a, i) => !(urlFlag >= 0 && (i === urlFlag || i === urlFlag + 1)));
  const [cmd, arg] = rest;
  if (!["snap", "errors", "eval", "click"].includes(cmd)) {
    throw new Error("用法: node scripts/gui-agent.mjs <snap|errors|eval|click> [参数] [--url URL]");
  }
  if ((cmd === "eval" || cmd === "click") && !arg) {
    throw new Error("命令 " + cmd + " 需要一个参数（JS 表达式或 CSS 选择器）");
  }
  return { cmd, arg, url };
}

function findChrome() {
  const bin = CHROME_CANDIDATES.find((p) => existsSync(p));
  if (!bin) throw new Error("未找到 Chrome，请设置 CHROME_BIN 指向可执行文件");
  return bin;
}

async function waitForEndpoint(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastErr = null;
  while (Date.now() < deadline) {
    try {
      const res = await fetch("http://127.0.0.1:" + port + "/json/version");
      if (res.ok) return;
    } catch (err) {
      lastErr = err;
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error("Chrome 调试端口 " + port + " 未就绪: " + (lastErr?.message ?? "超时"));
}

async function createTarget(port, url) {
  const query = new URLSearchParams({ url });
  const res = await fetch("http://127.0.0.1:" + port + "/json/new?" + query, { method: "PUT" });
  if (!res.ok) throw new Error("创建调试 target 失败: HTTP " + res.status);
  const target = await res.json();
  if (!target.webSocketDebuggerUrl) throw new Error("target 缺少 webSocketDebuggerUrl");
  return target;
}

class CdpConnection {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 0;
    this.pending = new Map();
    this.listeners = new Map();
  }

  connect() {
    return new Promise((res, rej) => {
      this.ws = new WebSocket(this.wsUrl);
      this.ws.onopen = res;
      this.ws.onerror = () => rej(new Error("CDP WebSocket 连接失败"));
      this.ws.onmessage = (msg) => this.handleMessage(JSON.parse(msg.data));
    });
  }

  handleMessage(msg) {
    if (msg.id !== undefined && this.pending.has(msg.id)) {
      const { res, rej } = this.pending.get(msg.id);
      this.pending.delete(msg.id);
      if (msg.error) rej(new Error(msg.error.message + "（CDP " + msg.id + "）"));
      else res(msg.result);
    } else if (msg.method) {
      for (const cb of this.listeners.get(msg.method) ?? []) cb(msg.params);
    }
  }

  send(method, params = {}) {
    const id = ++this.nextId;
    return new Promise((res, rej) => {
      this.pending.set(id, { res, rej });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  on(method, cb) {
    if (!this.listeners.has(method)) this.listeners.set(method, []);
    this.listeners.get(method).push(cb);
  }

  close() {
    this.ws.close();
  }
}

async function withPage(opts, fn) {
  const profileDir = mkdtempSync(join(tmpdir(), "gui-agent-"));
  const chrome = spawn(findChrome(), [
    "--headless=new",
    "--remote-debugging-port=" + DEBUG_PORT,
    "--user-data-dir=" + profileDir,
    "--no-first-run",
    "--no-default-browser-check",
    "--window-size=1440,900",
    "about:blank",
  ], { stdio: "ignore" });
  try {
    await waitForEndpoint(DEBUG_PORT, LOAD_TIMEOUT_MS);
    const cdp = await connectPage(opts.url + "/#/");
    try {
      return await fn(cdp);
    } finally {
      cdp.close();
    }
  } finally {
    chrome.kill("SIGKILL");
    // Chrome 收杀后可能仍在写 profile，稍候清理；清不掉只告警不影响结果。
    await new Promise((r) => setTimeout(r, 300));
    try {
      rmSync(profileDir, { recursive: true, force: true, maxRetries: 3 });
    } catch (err) {
      console.error("[gui-agent] 临时 profile 清理失败（不影响结果）:", profileDir, err.message);
    }
  }
}

async function connectPage(url) {
  const target = await createTarget(DEBUG_PORT, url);
  const cdp = new CdpConnection(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Runtime.enable");
  await cdp.send("Log.enable");
  await cdp.send("Page.enable");
  // 先挂 load 监听再导航，避免 target 已加载导致事件错失。
  const loaded = new Promise((res, rej) => {
    const timer = setTimeout(() => rej(new Error("页面加载超时")), LOAD_TIMEOUT_MS);
    cdp.on("Page.loadEventFired", () => { clearTimeout(timer); res(); });
  });
  await cdp.send("Page.navigate", { url });
  await loaded;
  await new Promise((r) => setTimeout(r, SETTLE_MS));
  return cdp;
}

async function evaluate(cdp, expression) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails) {
    const detail = result.exceptionDetails.exception?.description ?? "未知异常";
    throw new Error("页面内执行失败: " + detail);
  }
  return result.result.value;
}

async function cmdSnap(cdp, outPath) {
  const shot = await cdp.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: true,
  });
  const out = resolve(outPath ?? ".gui-agent/last.png");
  mkdirSync(resolve(".gui-agent"), { recursive: true });
  writeFileSync(out, Buffer.from(shot.data, "base64"));
  console.log(out);
}

async function cmdErrors(cdp) {
  const collected = { console: [], exceptions: [], appErrors: null };
  cdp.on("Runtime.consoleAPICalled", (p) => {
    collected.console.push(p.args.map((a) => a.value ?? a.description ?? "").join(" "));
  });
  cdp.on("Runtime.exceptionThrown", (p) => {
    const detail = p.exceptionDetails.exception?.description ?? p.exceptionDetails.text;
    collected.exceptions.push(detail);
  });
  const raw = await evaluate(cdp,
    "window.__P2P_AGENT__ ? JSON.stringify(window.__P2P_AGENT__.recentErrors()) : null");
  collected.appErrors = typeof raw === "string" ? JSON.parse(raw) : raw;
  console.log(JSON.stringify(collected, null, 2));
}

async function cmdEval(cdp, expression) {
  console.log(JSON.stringify(await evaluate(cdp, expression), null, 2));
}

async function cmdClick(cdp, selector) {
  const expression =
    "(function(){ const el = document.querySelector(" + JSON.stringify(selector) + ");" +
    " if (!el) throw new Error(" + JSON.stringify("选择器未命中: " + selector) + ");" +
    " el.click(); return true; })()";
  await evaluate(cdp, expression);
  console.log("clicked");
}

async function main() {
  const { cmd, arg, url } = parseArgs(process.argv.slice(2));
  const handlers = { snap: cmdSnap, errors: cmdErrors, eval: cmdEval, click: cmdClick };
  await withPage({ url }, (cdp) => handlers[cmd](cdp, arg));
}

main().catch((err) => {
  console.error("[gui-agent] 失败:", err.message);
  process.exit(1);
});
