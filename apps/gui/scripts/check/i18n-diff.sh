#!/usr/bin/env bash
# 机械对比 zh-CN/en-US locale key 集合，不一致 exit 1（G-B2 门禁）
# 任意 cwd 可执行：路径基于脚本自身定位。locale 文件结构必须是单对象字面量
# + 文件末尾 export default（同构自 i18n/index.ts 的资源装载）。
set -euo pipefail

GUI_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
export GUI_DIR

node <<'EOF'
const fs = require("fs");
const path = require("path");

const dir = path.join(process.env.GUI_DIR, "src", "i18n", "locales");

function keysOf(file, constName) {
  let src = fs.readFileSync(file, "utf8");
  src = src.replace(/^import[^\n]*\n\n?/, "");
  src = src.replace(
    new RegExp("^const " + constName + "(\\s*:\\s*typeof\\s+\\w+)?\\s*=\\s*"),
    "module.exports = ",
  );
  src = src.replace(/\nexport default \w+;\s*$/, "");
  const m = { exports: {} };
  new Function("module", "exports", src)(m, m.exports);
  const out = [];
  (function walk(obj, prefix) {
    for (const [k, v] of Object.entries(obj)) {
      const p = prefix ? prefix + "." + k : k;
      if (v !== null && typeof v === "object") walk(v, p);
      else out.push(p);
    }
  })(m.exports, "");
  return out.sort();
}

const zh = keysOf(path.join(dir, "zh-CN.ts"), "zhCN");
const en = keysOf(path.join(dir, "en-US.ts"), "enUS");
const onlyZh = zh.filter((k) => !en.includes(k));
const onlyEn = en.filter((k) => !zh.includes(k));

if (onlyZh.length > 0 || onlyEn.length > 0) {
  console.error("i18n-diff: key 集合不一致");
  if (onlyZh.length) console.error("  仅 zh-CN: " + onlyZh.join(", "));
  if (onlyEn.length) console.error("  仅 en-US: " + onlyEn.join(", "));
  process.exit(1);
}
console.log("i18n-diff: PASS（zh=" + zh.length + " en=" + en.length + "）");
EOF
