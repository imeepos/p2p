// wt5 驱动代理：5173 收 webview 请求，fetch 转发 vite(5174) 并向 HTML 注入驱动脚本。
// 仅测试夹具：不改仓库任何文件；端点协议与 wt3b 完全一致（driver.js 原样复用）。
import http from "node:http";
import { readFileSync } from "node:fs";

const UP = "http://127.0.0.1:5174";
const driver = () => readFileSync(new URL("./driver.js", import.meta.url), "utf8");

const cmds = [];
const results = new Map();

const server = http.createServer(async (req, res) => {
  const u = new URL(req.url, "http://x");
  if (u.pathname === "/__wt3b_driver.js") {
    res.writeHead(200, { "content-type": "text/javascript" });
    return res.end(driver());
  }
  if (u.pathname === "/__wt3b_cmd" && req.method === "GET") {
    const inst = u.searchParams.get("inst");
    const mine = cmds.filter((c) => c.inst === inst && !results.has(c.id));
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify(mine));
  }
  if (u.pathname === "/__wt3b_result" && req.method === "POST") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => {
      const j = JSON.parse(b || "{}");
      results.set(j.id, j);
      res.writeHead(200);
      res.end("ok");
    });
    return;
  }
  if (u.pathname === "/__wt3b_send" && req.method === "POST") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => {
      cmds.push(JSON.parse(b));
      res.writeHead(200);
      res.end("ok");
    });
    return;
  }
  if (u.pathname === "/__wt3b_results" && req.method === "GET") {
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify([...results.values()]));
  }
  // 常规转发：fetch 上游，HTML 注入驱动脚本标签
  try {
    const headers = { ...req.headers, host: "127.0.0.1:5174" };
    delete headers["if-none-match"];
    delete headers["if-modified-since"];
    delete headers["content-length"];
    const body =
      req.method === "GET" || req.method === "HEAD"
        ? undefined
        : await new Response(req).arrayBuffer();
    const up = await fetch(UP + req.url, {
      method: req.method,
      headers,
      body,
      redirect: "manual",
    });
    const type = up.headers.get("content-type") || "";
    let buf = Buffer.from(await up.arrayBuffer());
    if (type.includes("text/html")) {
      const html = buf.toString("utf8");
      const injected = html.replace(
        "</head>",
        `<script>window.__WT3B_INJECTED__=true;</script><script src="/__wt3b_driver.js"></script></head>`,
      );
      buf = Buffer.from(injected, "utf8");
    }
    const out = { "content-type": type, "cache-control": "no-store" };
    res.writeHead(up.status, out);
    res.end(buf);
  } catch (e) {
    res.writeHead(502);
    res.end(String(e));
  }
});
server.listen(5173, "127.0.0.1", () => console.log("wt5 driver proxy on 5173 -> 5174"));
