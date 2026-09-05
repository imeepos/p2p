#!/usr/bin/env node
// headless-metrics.mjs — 无头 Chrome CDP 页面度量（验收滚动/分组指标的正式入库工具）。
// 背景：无头验收聊天页时曾三次内联手写临时 CDP 脚本、两次嵌套转义翻车，一次入库复用。
// 零第三方依赖：Node 内置 fetch + WebSocket + CDP 原生协议，不依赖项目 node_modules。
//
// 用法：
//   1) 起带调试端口的 Chrome（headless/有头均可，独立 user-data-dir 防串扰）：
//      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' --headless=new \
//        --remote-debugging-port=9222 --user-data-dir=/tmp/dsh-cdp about:blank
//   2) 度量：
//      node scripts/ops/headless-metrics.mjs [URL] [--port N] [--out FILE] [--screenshot FILE] [--wait MS]
//      URL 缺省 http://localhost:5173/#/chat；--out 缺省把完整指标 JSON 打到 stdout；
//      --wait 为页面 load 后等待 SPA 渲染的毫秒数（缺省 1500）。
//   3) 指标：pageScrollV/pageScrollH 文档竖/横滚域；composerRect/composerInViewport
//      聊天输入框（[data-testid="chat-input"]）；slimScrollAreas .scroll-slim 滚域清单。
// 退出码：0 成功；2 CDP 连接失败；3 页面评估失败；4 参数非法。
import { writeFileSync } from 'node:fs';

function parseArgs(argv) {
  const opts = { url: 'http://localhost:5173/#/chat', port: 9222, out: null, screenshot: null, wait: 1500 };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--port') opts.port = Number(argv[++i]);
    else if (a === '--out') opts.out = argv[++i];
    else if (a === '--screenshot') opts.screenshot = argv[++i];
    else if (a === '--wait') opts.wait = Number(argv[++i]);
    else if (a.startsWith('--')) die(4, '未知参数 ' + a);
    else rest.push(a);
  }
  if (rest.length > 1) die(4, 'URL 只能传一个');
  if (rest.length === 1) opts.url = rest[0];
  if (!Number.isFinite(opts.port) || !Number.isFinite(opts.wait) || opts.wait < 0) die(4, '--port/--wait 需非负数字');
  return opts;
}

function die(code, msg) { console.error("headless-metrics: " + msg); process.exit(code); }
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class Cdp {
  constructor(ws) {
    this.ws = ws; this.id = 0; this.pending = new Map(); this.listeners = new Map();
    ws.addEventListener('message', (ev) => this.onMessage(JSON.parse(ev.data)));
  }
  onMessage(msg) {
    if (msg.id && this.pending.has(msg.id)) {
      const { resolve, reject } = this.pending.get(msg.id); this.pending.delete(msg.id);
      msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result); return;
    }
    if (msg.method && this.listeners.has(msg.method)) for (const fn of this.listeners.get(msg.method)) fn(msg.params);
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  on(event, fn) {
    if (!this.listeners.has(event)) this.listeners.set(event, []);
    this.listeners.get(event).push(fn);
  }
  once(event, timeoutMs, label) {
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(label + "（超时 " + timeoutMs + "ms）")), timeoutMs);
      this.on(event, (p) => { clearTimeout(t); resolve(p); });
    });
  }
}

async function waitOpen(ws, timeoutMs) {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { try { ws.close(); } catch {} reject(new Error("WebSocket 连接超时")); }, timeoutMs);
    ws.addEventListener('open', () => { clearTimeout(t); resolve(); }, { once: true });
    ws.addEventListener('error', () => { clearTimeout(t); reject(new Error('WebSocket 连接失败')); }, { once: true });
  });
}

async function newTarget(port) {
  const base = 'http://127.0.0.1:' + port;
  const url = base + '/json/new?about:blank';
  let res = await fetch(url, { method: "PUT" }).catch(() => null);   // Chrome 111+ 只收 PUT
  if (!res || !res.ok) res = await fetch(url).catch(() => null);     // 旧版兜底 GET
  if (!res || !res.ok) return null;
  return res.json();
}

// 页面内度量表达式：不得依赖项目代码，只认通用 testid/class 契约
const METRICS_EXPR = [
  "(() => {",
  "  const de = document.documentElement;",
  "  const vp = { w: window.innerWidth, h: window.innerHeight };",
  "  const composer = document.querySelector('[data-testid=\"chat-input\"]');",
  "  let composerRect = null, composerInViewport = false;",
  "  if (composer) {",
  "    const r = composer.getBoundingClientRect();",
  "    composerRect = { x: r.x, y: r.y, width: r.width, height: r.height, top: r.top, left: r.left, bottom: r.bottom, right: r.right };",
  "    composerInViewport = r.width > 0 && r.height > 0 && r.bottom > 0 && r.top < vp.h && r.right > 0 && r.left < vp.w;",
  "  }",
  "  const slimScrollAreas = Array.from(document.querySelectorAll('.scroll-slim')).map((el, i) => ({",
  "    index: i,",
  "    testid: el.getAttribute('data-testid'),",
  "    tag: el.tagName.toLowerCase(),",
  "    clientHeight: el.clientHeight,",
  "    scrollHeight: el.scrollHeight,",
  "    scrollableV: el.scrollHeight > el.clientHeight,",
  "  }));",
  "  return { viewport: vp, pageScrollV: de.scrollHeight - de.clientHeight, pageScrollH: de.scrollWidth - de.clientWidth, composerRect, composerInViewport, slimScrollAreas };",
  "})()"
].join("\n");

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const target = await newTarget(opts.port);
  if (!target || !target.webSocketDebuggerUrl) die(2, "连不上 CDP 端口 " + opts.port + "（先用 --remote-debugging-port 起 Chrome）");
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await waitOpen(ws, 8000);
  const cdp = new Cdp(ws);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  const loaded = cdp.once("Page.loadEventFired", 20000, "页面加载超时 " + opts.url);
  await cdp.send("Page.navigate", { url: opts.url });
  await loaded;
  await sleep(opts.wait);
  const res = await cdp.send("Runtime.evaluate", { expression: METRICS_EXPR, returnByValue: true });
  if (!res || res.exceptionDetails) {
    const d = res && res.exceptionDetails;
    die(3, "页面评估失败：" + (d ? (d.exception && d.exception.description || d.text) : "空结果"));
  }
  const payload = { capturedAt: new Date().toISOString(), url: opts.url, ...res.result.value };
  const json = JSON.stringify(payload, null, 2) + "\n";
  if (opts.screenshot) {
    const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
    writeFileSync(opts.screenshot, Buffer.from(shot.data, "base64"));
    console.error("headless-metrics: screenshot -> " + opts.screenshot);
  }
  if (opts.out) {
    writeFileSync(opts.out, json);
    console.error("headless-metrics: metrics -> " + opts.out);
  } else {
    process.stdout.write(json);
  }
  console.log("headless-metrics: OK pageScrollV=" + payload.pageScrollV
    + " pageScrollH=" + payload.pageScrollH
    + " composerInViewport=" + payload.composerInViewport
    + " slimScrollAreas=" + payload.slimScrollAreas.length);
  ws.close();
  process.exit(0);
}

main().catch((e) => die(2, String(e && e.message || e)));
