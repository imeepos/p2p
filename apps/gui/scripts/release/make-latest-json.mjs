#!/usr/bin/env node
// Node 脚本跑在 CI（无前端全局）：eslint 按 GUI 工程的浏览器全局检查，此处豁免 no-undef。
/* eslint-disable no-undef */
// 生成 Tauri updater 清单 latest.json（契约 v8 加法，G-U3）。
// 输入：CI release job 下载的 artifacts 目录（download-artifact@v4 每产物一子目录）。
// 规则：四平台签名增量包缺一即失败——宁可发布失败，不可产出残缺清单
// 破坏已装客户端的更新；macOS 双架构产物同名，就地改名加架构后缀再上传
// （签名只覆盖文件内容，与文件名无关）；端点固定 releases/latest/download/latest.json。
import { readdirSync, statSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

function fail(msg) {
  console.error("latest-json: FAIL " + msg);
  process.exit(1);
}

function parseArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i += 2) {
    if (!argv[i].startsWith("--") || argv[i + 1] === undefined) {
      fail("参数非法，期望 --artifacts <dir> --tag <client-vX.Y.Z> --repo <owner/name>");
    }
    args[argv[i].slice(2)] = argv[i + 1];
  }
  for (const k of ["artifacts", "tag", "repo"]) {
    if (!args[k]) fail("缺少 --" + k);
  }
  return args;
}

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

// 按平台匹配增量包与签名（成对出现，缺失/重复即失败）
function pick(files, dir, patterns) {
  const hit = (re) => files.filter((f) => f.startsWith(dir) && re.test(f));
  for (const { artifact, sig } of patterns) {
    const bins = hit(artifact);
    const sigs = hit(sig);
    if (bins.length === 0) continue;
    if (bins.length > 1) fail(dir + " 命中多个增量包: " + bins.join(", "));
    if (sigs.length !== 1) fail(dir + " 签名缺失或重复: " + sigs.join(", "));
    return { bin: bins[0], sig: sigs[0] };
  }
  return null;
}

const args = parseArgs(process.argv);
const m = /^client-v(\d+\.\d+\.\d+)$/.exec(args.tag);
if (!m) fail("tag 必须形如 client-vX.Y.Z: " + args.tag);
const version = m[1];

let entries;
try {
  entries = readdirSync(args.artifacts)
    .map((name) => join(args.artifacts, name))
    .filter((p) => statSync(p).isDirectory());
} catch (e) {
  fail("artifacts 目录不可读: " + args.artifacts + " (" + e.message + ")");
}
const findDir = (suffix) => {
  const dir = entries.find((d) => d.endsWith(suffix));
  if (!dir) fail("缺少产物目录 *" + suffix);
  return dir;
};
const allFiles = entries.flatMap((dir) => walk(dir));

const platforms = {};
// macOS：两架构同名 tar 包，改名规避 release 资产重名
for (const [suffix, platform, arch] of [
  ["p2p-console-macos-latest", "darwin-aarch64", "aarch64"],
  ["p2p-console-macos-15-intel", "darwin-x86_64", "x86_64"],
]) {
  const dir = findDir(suffix);
  const picked = pick(allFiles, dir, [
    { artifact: /\.app\.tar\.gz$/, sig: /\.app\.tar\.gz\.sig$/ },
  ]);
  if (!picked) fail(suffix + " 无 .app.tar.gz 增量包（签名私钥未配置?）");
  const newName = "p2p-console-" + arch + ".app.tar.gz";
  renameSync(picked.bin, join(dir, newName));
  renameSync(picked.sig, join(dir, newName + ".sig"));
  platforms[platform] = { file: newName, sigPath: join(dir, newName + ".sig") };
}

// Linux AppImage / Windows NSIS：文件名自带版本与架构，无需改名
const linuxDir = findDir("p2p-console-linux");
const linuxPick = pick(allFiles, linuxDir, [
  { artifact: /\.AppImage\.tar\.gz$/, sig: /\.AppImage\.tar\.gz\.sig$/ },
]);
if (!linuxPick) fail("linux 无 AppImage 增量包");
platforms["linux-x86_64"] = { file: linuxPick.bin.split("/").pop(), sigPath: linuxPick.sig };

const winDir = findDir("p2p-console-windows");
const winPick = pick(allFiles, winDir, [
  { artifact: /-setup\.nsis\.zip$/, sig: /-setup\.nsis\.zip\.sig$/ },
  { artifact: /-setup\.exe$/, sig: /-setup\.exe\.sig$/ },
]);
if (!winPick) fail("windows 无 NSIS 增量包");
platforms["windows-x86_64"] = { file: winPick.bin.split("/").pop(), sigPath: winPick.sig };

const manifest = {
  version,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  platforms: Object.fromEntries(
    Object.entries(platforms).map(([platform, p]) => [
      platform,
      {
        signature: readFileSync(p.sigPath, "utf8").trim(),
        url: "https://github.com/" + args.repo + "/releases/download/" + args.tag + "/" + p.file,
      },
    ]),
  ),
};

const out = join(args.artifacts, "latest.json");
writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log("latest-json: PASS " + out + " platforms=" + Object.keys(manifest.platforms).join(","));
