// wt3b webview 驱动：自识别实例（dataDir），轮询命令队列，执行后回传结果。
// 命令面：invoke（Tauri 命令）/ nav / fill_open（远程访问视图表单填充+点击）。
const invoke = (cmd, args) => Promise.race([window.__TAURI_INTERNALS__.invoke(cmd, args), new Promise((_,rej)=>setTimeout(()=>rej(new Error('ipc-timeout')),3000))]);

const post = (path, body) =>
  fetch(path, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });

function setNativeValue(el, value) {
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
  setter.call(el, value);
  el.dispatchEvent(new Event("input", { bubbles: true }));
}

async function fillOpen(args) {
  location.hash = "#/remote-access";
  await new Promise((r) => setTimeout(r, 600));
  const url = document.querySelector("#tunnel-dsh-url");
  const peer = document.querySelector("#tunnel-peer");
  if (!url || !peer) throw new Error("远程访问视图表单未找到（路由未挂载?）");
  setNativeValue(url, args.url);
  setNativeValue(peer, args.peer);
  await new Promise((r) => setTimeout(r, 200));
  const card = url.closest("[data-slot=card], .rounded-lg, .space-y-4") ?? document;
  const btn = [...card.querySelectorAll("button")].find((b) => !b.disabled);
  const candidates = [...document.querySelectorAll("button")].map((b) => b.textContent.trim());
  (window.__WT3B_DEBUG__ ||= []).push(candidates);
  if (!btn) throw new Error("未找到可用按钮: " + candidates.join("|"));
  btn.click();
  await new Promise((r) => setTimeout(r, 1500));
  return { clicked: btn.textContent.trim(), phaseBadge: document.querySelector("[class*=badge]")?.textContent ?? null };
}

async function run(fn, args) {
  if (fn === "invoke") return { value: await invoke(args.cmd, args.args ?? {}) };
  if (fn === "nav") { location.hash = "#/" + args.route; await new Promise((r) => setTimeout(r, 400)); return { hash: location.hash }; }
  if (fn === "fill_open") return await fillOpen(args);
  if (fn === "text") {
    await new Promise((r) => setTimeout(r, 200));
    return { badge: document.querySelector(".space-y-4 .flex.items-center.gap-2 span")?.textContent ?? null, body: document.body.innerText.slice(0, 3000) };
  }
  throw new Error("未知 fn " + fn);
}

async function main() {
  let inst = "unknown";
  let dir = "";
  for (let i = 0; i < 2 && inst === "unknown"; i++) {
    try {
      const cfg = await invoke("config_get");
      dir = cfg.dataDir || "";
      inst = dir.includes("node-a") ? "a" : dir.includes("node-b") ? "b" : "unknown";
    } catch (e) {
      console.error("[wt3b] identify 失败", e);
    }
    if (inst === "unknown") await new Promise((r) => setTimeout(r, 1000));
  }
  window.__WT3B_INST__ = inst;
  try { await Promise.race([post("/__wt3b_result", { id: "boot", inst, fn: "boot", result: { inst, dir, href: location.href } }), new Promise(r=>setTimeout(r,2000))]); } catch(e) { console.error("[wt5] boot post 失败", e); }
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
      console.error("[wt3b] 队列轮询失败", e);
    }
    await new Promise((r) => setTimeout(r, 700));
  }
}
main();
