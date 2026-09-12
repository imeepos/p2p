// wt3b 驱动代理：5173 收 webview 请求，转发到 vite(5174) 并向 index.html 注入驱动脚本。
// 仅测试夹具：不修改仓库任何文件；注入脚本存于本目录。
import http from "node:http";
import { readFileSync } from "node:fs";

const UP = "http://127.0.0.1:5174";
const driverPath = new URL("./driver.js", import.meta.url);
const driver = () => readFileSync(driverPath, "utf8");

const cmds = []; // {id,inst,fn,args}
const results = new Map(); // id -> {inst,fn,result,error,at}

function proxy(req, res) {
  const url = new URL(req.url, UP);
  const fwd = { ...req.headers, host: "localhost:5174" };
  delete fwd["if-none-match"];
  delete fwd["if-modified-since"];
  const r = http.request(url, { method: req.method, headers: fwd }, (ur) => {
    const chunks = [];
    ur.on("data", (c) => chunks.push(c));
    ur.on("end", () => {
      let body = Buffer.concat(chunks);
      const ct = ur.headers["content-type"] || "";
      if (req.url === "/" || req.url.startsWith("/?") || (req.url.startsWith("/index.html"))) {
        const html = body.toString("utf8");
        const injected = html.replace(
          "</head>",
          `<script>window.__WT3B_INJECTED__=true;</script><script src="/__wt3b_driver.js"></script></head>`,
        );
        body = Buffer.from(injected, "utf8");
      }
      res.writeHead(ur.statusCode, { ...ur.headers, "content-length": body.length, "cache-control": "no-store" });
      res.end(body);
    });
  });
  req.pipe(r);
  r.on("error", (e) => { res.writeHead(502); res.end(String(e)); });
}

const server = http.createServer((req, res) => {
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
  proxy(req, res);
});
server.listen(5173, "127.0.0.1", () => console.log("wt3b driver proxy on 5173 -> 5174"));
