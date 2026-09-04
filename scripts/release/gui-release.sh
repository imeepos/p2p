#!/usr/bin/env bash
# GUI 本地一键构建+冒烟（W2）：三处版本一致 -> tauri build（app+dmg）-> 产物冒烟 -> updater latest.json 校验
# 用法：bash scripts/release/gui-release.sh
# 版本口径复用 scripts/check/version.sh：apps/gui/package.json / src-tauri/tauri.conf.json / Cargo.toml 三处一致。
# 签名：环境或 .env 的 TAURI_SIGNING_PRIVATE_KEY_PATH 指向 minisign 私钥（无密码）时按 signed 构建，
#       产出 updater 产物（.app.tar.gz/.sig/latest.json）并做结构校验；无键时以 --config 关
#       createUpdaterArtifacts 构建 unsigned，显式标注不算失败（CI 签名流水线另行负责真签发）。
# 产物在 apps/gui/src-tauri/target/release/bundle/（src-tauri/.gitignore /target 已覆盖，不入库）。
# 末行输出 W2-RELEASE-OK；构建或冒烟失败一律非 0 退出。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GUI="$ROOT/apps/gui"
TAURI="$GUI/src-tauri"
BUNDLE="$TAURI/target/release/bundle"
BUILD_LOG="$ROOT/.gui-release-build.log"

APP_NAME="$(grep -m1 '"productName"' "$TAURI/tauri.conf.json" | sed 's/.*: *"//;s/".*//')"
VERSION="$(grep -m1 '"version"' "$GUI/package.json" | sed 's/.*: *"//;s/".*//')"
MODE="unsigned"

fail() { echo "gui-release: FAIL $*" >&2; exit 1; }
log() { echo "gui-release: $*"; }

version_gate() {
  bash "$ROOT/scripts/check/version.sh" \
    || fail "三处版本一致性校验未过（口径同 scripts/check/release.sh）"
  [ -n "$APP_NAME" ] || fail "tauri.conf.json 未解析到 productName"
  [ -n "$VERSION" ] || fail "package.json 未解析到 version"
  log "版本口径：$VERSION（productName=$APP_NAME，三处一致）"
}

# 装载 updater 签名私钥；返回 0=signed / 1=unsigned。配置存在但文件缺失属硬错误。
load_signing_key() {
  [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && return 0
  local env_file="$ROOT/.env" key_path=""
  [ -f "$env_file" ] || return 1
  key_path="$(grep -E '^TAURI_SIGNING_PRIVATE_KEY_PATH=' "$env_file" | tail -n1 | cut -d= -f2-)"
  key_path="$(printf '%s' "$key_path" | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//")"
  [ -n "$key_path" ] || return 1
  [ -f "$key_path" ] || fail "TAURI_SIGNING_PRIVATE_KEY_PATH 配置存在但文件缺失：$key_path"
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_path")"
  export TAURI_SIGNING_PRIVATE_KEY
  # 无密码私钥也必须显式置空密码：不设时 tauri 走 TTY 交互解密，无 TTY 即 os error 6
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
  return 0
}

ensure_frontend_deps() {
  [ -d "$GUI/node_modules" ] && return 0
  log "node_modules 缺失，先在 workspace 根 pnpm install..."
  (cd "$ROOT" && pnpm install --frozen-lockfile --prefer-offline) \
    || fail "pnpm install 失败（前端依赖未就位）"
}

run_build() {
  rm -rf "$BUNDLE"  # 幂等：清旧产物，冒烟只看本次构建，防脏 latest.json 误判
  local args=(build)
  if [ "$MODE" = "signed" ]; then
    log "2/5 tauri build（signed：TAURI_SIGNING_PRIVATE_KEY 已装载，产出 updater 产物）"
  else
    log "2/5 tauri build（unsigned：未发现签名私钥，--config 关 createUpdaterArtifacts；显式标注不算失败）"
    args+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
  fi
  (cd "$GUI" && pnpm tauri "${args[@]}") >"$BUILD_LOG" 2>&1 || {
    tail -n 40 "$BUILD_LOG" >&2
    fail "tauri build 失败（完整日志 $BUILD_LOG）"
  }
  tail -n 3 "$BUILD_LOG"
}

size_line() { printf '%s' "$(du -sh "$1" | cut -f1)"; }   # 目录与文件通吃，单行总量

# tauri v2 本地只产 .app.tar.gz + .sig，latest.json 由发布方组装（社区实践口径）。
# 这里 signed 模式本地组装一份供 updater 流与 4/5 校验用；失败即硬错，不静默。
gen_latest_json() {
  [ "$MODE" = "signed" ] || return 0
  local sig tar url plat pub_date
  sig="$(find "$BUNDLE" -name '*.app.tar.gz.sig' -type f | head -n1)"
  [ -n "$sig" ] && [ -s "$sig" ] || fail "signed 构建未产出 .app.tar.gz.sig"
  tar="$(basename "$sig" .sig)"
  find "$BUNDLE" -name "$tar" -type f | grep -q . || fail ".sig 在位但被签产物缺失：$tar"
  url="$tar"
  plat="darwin-$(uname -m | sed 's/^arm64$/aarch64/')"
  pub_date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  jq -n --arg version "$VERSION" --arg pub_date "$pub_date" \
    --arg sig "$(cat "$sig")" --arg url "$url" --arg plat "$plat" \
    '{version: $version, pub_date: $pub_date,
      platforms: {($plat): {signature: $sig, url: $url}}}' \
    > "$BUNDLE/macos/latest.json" || fail "latest.json 组装失败（jq 非法输入?）"
  log "2/5 latest.json 组装 OK（$plat -> $url，签名复用 tauri .sig）"
}

smoke_app() {
  local app="$BUNDLE/macos/$APP_NAME.app" plist_ver bin
  [ -d "$app" ] || fail "缺 .app 产物：$app"
  [ -s "$app/Contents/Info.plist" ] || fail "缺 Info.plist：$app/Contents/Info.plist"
  plist_ver="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist" 2>/dev/null)" \
    || fail "Info.plist CFBundleShortVersionString 读取失败"
  [ "$plist_ver" = "$VERSION" ] || fail "Info.plist 版本 $plist_ver 与三处口径 $VERSION 不一致"
  bin="$(find "$app/Contents/MacOS" -maxdepth 1 -type f -size +0c | head -n1)"
  [ -n "$bin" ] || fail "主二进制缺失或为空：$app/Contents/MacOS"
  LC_ALL=C grep -aqF -- "$VERSION" "$bin" || fail "二进制内未匹配到 version 串 $VERSION：$bin"
  log "3/5 APP OK $(size_line "$app") Info.plist=$plist_ver 二进制含 $VERSION"
}

smoke_dmg() {
  local dmg
  dmg="$(ls -t "$BUNDLE"/dmg/*.dmg 2>/dev/null | head -n1 || true)"
  [ -n "$dmg" ] && [ -s "$dmg" ] || fail "缺 dmg 产物（$BUNDLE/dmg/*.dmg）"
  log "3/5 DMG OK $(size_line "$dmg") ($(stat -f%z "$dmg") bytes)"
}

# latest.json 结构校验：version/pub_date/platforms 必填，签名字段非空且被签产物在位；
# unsigned 路径无此文件，显式标注 SKIP 不算失败。
validate_latest_json() {
  local latest
  latest="$(find "$BUNDLE" -maxdepth 2 -name latest.json -type f | head -n1 || true)"
  if [ -z "$latest" ]; then
    [ "$MODE" = "signed" ] && fail "signed 构建未产出 latest.json（updater 产物缺失）"
    log "4/5 LATEST SKIP unsigned 构建（未启用 createUpdaterArtifacts），无 latest.json"
    return 0
  fi
  node -e '
    const fs = require("fs");
    const [path, want] = process.argv.slice(1);
    const j = JSON.parse(fs.readFileSync(path, "utf8"));
    const bad = (m) => { console.error("latest.json FAIL " + m); process.exit(1); };
    if (j.version !== want) bad("version " + j.version + " != " + want);
    if (!j.pub_date) bad("缺 pub_date");
    const p = j.platforms;
    if (!p || typeof p !== "object" || Object.keys(p).length === 0) bad("platforms 为空");
    for (const [k, v] of Object.entries(p)) {
      if (!v.signature || !String(v.signature).trim()) bad("platforms." + k + " 签名字段为空");
      if (!v.url) bad("platforms." + k + " 缺 url");
    }
    console.log("latest.json OK version=" + j.version + " platforms=" + Object.keys(p).join(","));
  ' "$latest" "$VERSION" || fail "latest.json 结构校验未过：$latest"
  local url base
  while read -r url; do
    [ -n "$url" ] || continue
    base="$(basename "$url")"
    find "$BUNDLE" -name "$base" -type f | grep -q . \
      || fail "latest.json 引用的产物不在位：$base"
    log "4/5 SIG OK $base（签名字段非空且产物在位）"
  done < <(node -e 'const j=require(process.argv[1]);for(const v of Object.values(j.platforms))console.log(v.url)' "$latest")
}

main() {
  log "1/5 版本三处一致校验"
  version_gate
  ensure_frontend_deps
  if load_signing_key; then
    MODE="signed"
  else
    log "未发现 TAURI_SIGNING_PRIVATE_KEY（环境/.env 均无），走 unsigned 构建"
  fi
  run_build
  gen_latest_json
  smoke_app
  smoke_dmg
  validate_latest_json
  log "5/5 SUMMARY mode=$MODE app=$BUNDLE/macos/$APP_NAME.app dmg=$BUNDLE/dmg log=$BUILD_LOG"
  echo "W2-RELEASE-OK"
}

main "$@"