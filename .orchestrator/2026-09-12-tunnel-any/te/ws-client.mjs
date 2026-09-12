// TE WS 双向往返客户端（node 24 内置 WebSocket）：文本帧 + 二进制帧各一次，
// 输出 echo 回包；close(1000) 触发服务端 close 事件。仅测试夹具。
const url = process.argv[2];
const log = (...a) => console.log(new Date().toISOString(), ...a);
const ws = new WebSocket(url);
ws.onopen = () => {
  log("open", url);
  ws.send("te-echo-text-" + Date.now());
};
ws.onmessage = (ev) => {
  if (typeof ev.data === "string") {
    log("text-echo", ev.data);
    ws.send(new Uint8Array([1, 2, 3, 250, 251, 252]));
    return;
  }
  const done = (buf) => {
    log("binary-echo", Buffer.from(buf).toString("hex"));
    ws.close(1000, "te-done");
  };
  if (ev.data.arrayBuffer) ev.data.arrayBuffer().then(done);
  else done(ev.data);
};
ws.onclose = (ev) => { log("close", ev.code, ev.reason || "(no reason)"); process.exit(0); };
ws.onerror = (e) => { log("error", String(e && e.message || e)); process.exit(1); };
setTimeout(() => { log("timeout-no-roundtrip"); process.exit(2); }, 8000);
