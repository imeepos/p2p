// TE webview 驱动（wt3b 协议兼容，wt5/driver.js 改版，仅测试夹具）：
// 自识别实例（dataDir），轮询命令队列，执行后回传。
// 命令面：invoke（Tauri 命令）/ nav / fill_generic（TD 通用服务表单填充+点击）/ text。
const invoke = (cmd, args) => Promise.race([window.__TAURI_INTERNALS__.invoke(cmd, args), new Promise((_,rej)=>setTimeout(()=>rej(new Error('ipc-timeout')),3000))]);

const post = (path, body) =>
  fetch(path, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });

function setNativeValue(el, value) {
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
  setter.call(el, value);
  el.dispatchEvent(new Event("input", { bubbles: true }));
}

async function fillGeneric(args) {
  location.hash = "#/remote-access";
  await new Promise((r) => setTimeout(r, 800));
  const port = document.querySelector("#tunnel-generic-target");
  const peer = document.querySelector("#tunnel-generic-peer");
  if (!port || !peer) throw new Error("通用服务表单未找到（TD 前端未加载?）");
  setNativeValue(port, args.port);
  setNativeValue(peer, args.peer);
  await new Promise((r) => setTimeout(r, 200));
  const card = port.closest("[data-slot=card], .rounded-lg, .space-y-4") ?? document;
  const btn = [...card.querySelectorAll("button")].find((b) => !b.disabled);
  (window.__TE_DEBUG__ ||= []).push([...document.querySelectorAll("button")].map((b) => b.textContent.trim()));
  if (!btn) throw new Error("未找到可用按钮");
  btn.click();
  await new Promise((r) => setTimeout(r, 2500));
  return { clicked: btn.textContent.trim(), body: document.body.innerText.slice(0, 4000) };
}

async function run(fn, args) {
  if (fn === "invoke") return { value: await invoke(args.cmd, args.args ?? {}) };
  if (fn === "nav") { location.hash = "#/" + args.route; await new Promise((r) => setTimeout(r, 500)); return { hash: location.hash }; }
  if (fn === "fill_generic") return await fillGeneric(args);
  if (fn === "text") {
    await new Promise((r) => setTimeout(r, 200));
    return { hash: location.hash, body: document.body.innerText.slice(0, 4000) };
  }
  throw new Error("未知 fn " + fn);
}

async function main() {
  let inst = "b";
  let dir = "";
  for (let i = 0; i < 2; i++) {
    try {
      const cfg = await invoke("config_get");
      dir = cfg.dataDir || "";
      inst = "b"; // TE 单实例（访侧 GUI）
      break;
    } catch (e) {
      console.error("[te] identify 失败", e);
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
  window.__TE_INST__ = inst;
  try { await Promise.race([post("/__wt3b_result", { id: "boot", inst, fn: "boot", result: { inst, dir, href: location.href } }), new Promise(r=>setTimeout(r,2000))]); } catch(e) { console.error("[te] boot post 失败", e); }
  for (;;) {
    try {
      const res = await fetch(`/__wt3b_cmd?inst=${inst}&t=${Date.now()}`);
      const list = await res.json();
      for (const c of list) {
        let out;
        try {
          out = { id: c.id, inst, fn: c.fn, result: await run(c.fn, c.args ?? {}) };
        } catch (e) {
          out = { id: c.id, inst, fn: c.fn, error: String((e && e.message) || e) };
        }
        await post("/__wt3b_result", out);
      }
    } catch (e) {
      console.error("[te] 队列轮询失败", e);
    }
    await new Promise((r) => setTimeout(r, 700));
  }
}
main();
