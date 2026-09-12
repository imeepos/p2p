// TE 静态注入服务器（wt5 static-injector 改版，仅测试夹具）：
// 伺服 apps/gui/dist（fb70de4f 构建，含 TD 通用表单）+ 注入驱动脚本 + wt3b 协议命令队列。
// 改动点：绑双栈（wt3c 教训：devUrl=localhost 解析 ::1 时仅 127.0.0.1 会失败）、
// 请求日志落 /tmp/te-injector-reqs.log（webview 加载注入页的证据）。
import http from "node:http";
import { readFileSync, existsSync, statSync, appendFileSync } from "node:fs";
import { join, extname, normalize } from "node:path";

const DIST = "/Users/imeepos/ext512/p2p/apps/gui/dist";
const DRIVER = new URL("./driver.js", import.meta.url);
const LOG = "/tmp/te-injector-reqs.log";
const MIME = {
  ".html": "text/html", ".js": "text/javascript", ".css": "text/css",
  ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png",
  ".woff": "font/woff", ".woff2": "font/woff2", ".ttf": "font/ttf",
  ".ico": "image/x-icon", ".map": "application/json", ".webp": "image/webp",
};

const cmds = [];
const results = new Map();

function logReq(req) {
  try {
    appendFileSync(LOG, `${new Date().toISOString()} ${req.method} ${req.url} ua=${(req.headers["user-agent"] || "").slice(0, 60)}\n`);
  } catch {}
}

function serveFile(res, path, inject) {
  const body = readFileSync(path);
  if (inject && path.endsWith(".html")) {
    const injected = body
      .toString("utf8")
      .replace(
        "</head>",
        `<script>window.__WT3B_INJECTED__=true;</script><script src="/__wt3b_driver.js"></script></head>`,
      );
    res.writeHead(200, { "content-type": "text/html", "cache-control": "no-store" });
    return res.end(injected);
  }
  res.writeHead(200, {
    "content-type": MIME[extname(path)] || "application/octet-stream",
    "cache-control": "no-store",
  });
  res.end(body);
}

const server = http.createServer((req, res) => {
  logReq(req);
  const u = new URL(req.url, "http://x");
  if (u.pathname === "/__wt3b_driver.js") {
    res.writeHead(200, { "content-type": "text/javascript" });
    return res.end(readFileSync(DRIVER));
  }
  if (u.pathname === "/__wt3b_cmd" && req.method === "GET") {
    const inst = u.searchParams.get("inst");
    return res.end(JSON.stringify(cmds.filter((c) => c.inst === inst && !results.has(c.id))));
  }
  if (u.pathname === "/__wt3b_result" && req.method === "POST") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => {
      const j = JSON.parse(b || "{}");
      results.set(j.id, j);
      res.writeHead(200); res.end("ok");
    });
    return;
  }
  if (u.pathname === "/__wt3b_send" && req.method === "POST") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => {
      cmds.push(JSON.parse(b));
      res.writeHead(200); res.end("ok");
    });
    return;
  }
  if (u.pathname === "/__wt3b_results" && req.method === "GET") {
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify([...results.values()]));
  }
  const rel = u.pathname === "/" ? "index.html" : normalize(u.pathname).slice(1);
  const path = join(DIST, rel);
  if (existsSync(path) && statSync(path).isFile()) {
    return serveFile(res, path, u.pathname === "/" || u.pathname.endsWith(".html"));
  }
  // SPA 回退
  if (!rel.includes(".")) {
    return serveFile(res, join(DIST, "index.html"), true);
  }
  res.writeHead(404); res.end("not found");
});
// 无 host 参数 = :: 双栈（IPv4 mapped），localhost 与 127.0.0.1 皆可达
server.listen(5173, () => console.log("te static injector on 5173 (dual-stack, dist)"));
