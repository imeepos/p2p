#!/usr/bin/env bash
# 版本一致性门禁：apps/gui 三处版本必须同值（0.1.1 发布事故后加入，见 docs/release-gates.md）
#   apps/gui/package.json / apps/gui/src-tauri/tauri.conf.json / apps/gui/src-tauri/Cargo.toml
# 用法：bash scripts/check/version.sh [期望版本]
#   无参：只校验三处互相一致；带参：额外要求三处都等于该值（release.sh 复用）
# 输出：成功 stdout 一行 PASS；失败 stderr 带 "version-check: FAIL" + 三处实际值，exit 1
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/release-gates.sh 用临时夹具驱动）
set -uo pipefail

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

PKG="$ROOT/apps/gui/package.json"
CONF="$ROOT/apps/gui/src-tauri/tauri.conf.json"
CARGO="$ROOT/apps/gui/src-tauri/Cargo.toml"

for f in "$PKG" "$CONF" "$CARGO"; do
  if [ ! -f "$f" ]; then
    echo "version-check: FAIL 缺少文件 $f（GUI 工程未就位?）" >&2
    exit 1
  fi
done

# JSON 两处取顶层首个 "version" 键（依赖块的键是包名，不与之冲突；-m1 锁定首个）
json_version() {
  grep -m1 -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$1" | sed 's/.*"\([^"]*\)"$/\1/'
}

# Cargo.toml 只认 [package] 小节内行首 version（依赖内联 version = "2" 不在小节内、也不行首）
cargo_version() {
  awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/ { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
      line = $0
      sub(/^version[[:space:]]*=[[:space:]]*"/, "", line)
      sub(/".*$/, "", line)
      print line
      exit
    }
  ' "$1"
}

V_PKG="$(json_version "$PKG")"
V_CONF="$(json_version "$CONF")"
V_CARGO="$(cargo_version "$CARGO")"

fail() {
  echo "version-check: FAIL $1" >&2
  {
    printf '  apps/gui/package.json              -> %s\n' "${V_PKG:-<未解析>}"
    printf '  apps/gui/src-tauri/tauri.conf.json -> %s\n' "${V_CONF:-<未解析>}"
    printf '  apps/gui/src-tauri/Cargo.toml      -> %s\n' "${V_CARGO:-<未解析>}"
  } >&2
  exit 1
}

[ -n "$V_PKG" ] && [ -n "$V_CONF" ] && [ -n "$V_CARGO" ] || fail "版本字段未全部解析（文件格式被改坏?）"

if [ "$V_PKG" != "$V_CONF" ] || [ "$V_PKG" != "$V_CARGO" ]; then
  fail "三处版本不一致（bump 时三处必须同值提交）"
fi

if [ "$#" -gt 0 ]; then
  [ "$V_PKG" = "$1" ] || fail "期望版本 $1，实际 $V_PKG"
fi

echo "version-check: PASS 三处版本一致：$V_PKG"
