// 本地 ACP WS 探针：node 原生 WebSocket 直连 acp-console 本地面，
// 走 initialize → session/new → prompt 三请求往返（console 是纯字节泵，
// 对端是 acp-echo-stub 时 prompt 行原样回写 = 同 id 应答往返即闭环）。
// 用法：node acp-local-ws-probe.mjs <wsUrl> <token> <agentPeer> <timeoutSecs>
// 成功输出 WS-PROBE-OK，失败退出码 1。
import process from "node:process";

const [wsUrl, token, agentPeer, timeoutSecs] = process.argv.slice(2);
if (!wsUrl || !token || !agentPeer) {
  console.error("usage: acp-local-ws-probe.mjs <wsUrl> <token> <agentPeer> [timeoutSecs]");
  process.exit(1);
}
const budgetMs = (Number(timeoutSecs) || 25) * 1000;
// console ready 行的 ws 字段缺省不带 scheme（addr 展示形如 127.0.0.1:PORT），此处归一化
const rawWs = /^[a-z][a-z0-9+.-]*:\/\//.test(wsUrl) ? wsUrl : "ws://" + wsUrl;
const url = new URL(rawWs);
url.searchParams.set("token", token);
url.searchParams.set("peer", agentPeer);

let buffer = "";
const pending = new Map();
let nextId = 1;
let probeDone = false;

function onLine(line) {
  if (!line.trim()) return;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return;
  }
  if (msg.id !== undefined && pending.has(msg.id)) {
    const settle = pending.get(msg.id);
    pending.delete(msg.id);
    settle(msg);
  }
}

function send(ws, obj) {
  ws.send(JSON.stringify(obj) + "\n");
}

function request(ws, method, params, label) {
  const id = nextId++;
  const promise = new Promise((resolve, reject) => {
    pending.set(id, resolve);
    setTimeout(() => {
      if (pending.has(id)) {
        pending.delete(id);
        reject(new Error(label + " 超时无应答"));
      }
    }, budgetMs);
  });
  send(ws, { jsonrpc: "2.0", id, method, params });
  return promise;
}

const ws = new WebSocket(url.toString());
const bail = (message) => {
  if (probeDone) return;
  probeDone = true;
  console.error(message);
  process.exit(1);
};

const watchdog = setTimeout(() => bail("WS-PROBE 总预算超时"), budgetMs + 5_000);

ws.addEventListener("open", async () => {
  try {
    const init = await request(
      ws,
      "initialize",
      { protocolVersion: 1, clientCapabilities: {} },
      "initialize",
    );
    // 回应可能是真应答（result）或 echo 桩的原样回显（同 id 的请求行）：
    // 两种形态都证明请求穿过 GUI→console→P2P→agent→回 全链。
    const initIsReply = init.result !== undefined || init.method !== undefined;
    if (!initIsReply) throw new Error("initialize 应答形态不可判: " + JSON.stringify(init));
    console.log(
      "[ws] initialize OK:",
      JSON.stringify(init.result?.agentInfo ?? { echo: true }),
    );
    const created = await request(ws, "session/new", { cwd: process.cwd(), mcpServers: [] }, "session/new");
    const sessionId = created.result?.sessionId ?? "echo-session";
    console.log("[ws] session/new OK:", sessionId);
    const prompt = await request(
      ws,
      "session/prompt",
      { sessionId, prompt: [{ type: "text", text: "smoke-roundtrip" }] },
      "session/prompt",
    );
    if (prompt.error) throw new Error("prompt 应答错误: " + JSON.stringify(prompt.error));
    console.log("[ws] prompt 往返 OK:", JSON.stringify(prompt.result ?? {}));
    clearTimeout(watchdog);
    probeDone = true;
    console.log("WS-PROBE-OK");
    process.exit(0);
  } catch (err) {
    bail(String(err));
  }
});
ws.addEventListener("message", (event) => {
  buffer += typeof event.data === "string" ? event.data : String(event.data);
  let at;
  while ((at = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, at);
    buffer = buffer.slice(at + 1);
    onLine(line);
  }
});
ws.addEventListener("close", (event) => {
  bail("WS 提前关闭 code=" + event.code + (buffer ? " 残留=" + buffer.slice(0, 120) : ""));
});
ws.addEventListener("error", () => bail("WS 连接失败"));
