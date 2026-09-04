#!/usr/bin/env bash
# p2pctl 发布产物构建：release 构建 + 冒烟断言（GUI 打包分发另行立项，本脚本不碰）
# 用法：bash scripts/release/p2pctl-release.sh
# 产物：apps/cli/target/release/p2pctl（.gitignore 已覆盖，不入库）
# 末行输出 R2-RELEASE-OK；构建或冒烟失败一律非 0 退出
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CLI_DIR="$ROOT/apps/cli"
BIN="$CLI_DIR/target/release/p2pctl"
# 冒烟临时目录：必须全局——EXIT trap 在函数返回后才触发，local 变量在 set -u 下会 unbound
SMOKE_TMP=""

fail() { echo "p2pctl-release: FAIL $*" >&2; exit 1; }
log() { echo "p2pctl-release: $*"; }

# JSON 合法性断言（python3 标准库解析；无 python3 显式失败，不静默放行）
assert_valid_json() {
  local label="$1" payload="$2"
  command -v python3 >/dev/null || fail "python3 不可用，无法校验 JSON"
  printf '%s' "$payload" | python3 -c 'import json,sys; json.load(sys.stdin)' \
    || fail "$label 输出不是合法 JSON: $payload"
}

build() {
  [ -d "$CLI_DIR" ] || fail "缺少 $CLI_DIR"
  log "1/4 cargo build --release (apps/cli)"
  (cd "$CLI_DIR" && cargo build --release) || fail "cargo build --release 失败"
  [ -x "$BIN" ] || fail "产物缺失或不可执行: $BIN"
}

smoke() {
  local out
  log "2/4 smoke: --version"
  out="$("$BIN" --version)" || fail "--version 退出码非 0"
  [[ "$out" == p2pctl* ]] || fail "--version 输出异常: $out"
  echo "  $out"

  SMOKE_TMP="$(mktemp -d)" || fail "mktemp 失败"
  trap 'rm -rf "$SMOKE_TMP"' EXIT

  log "3/4 smoke: node status --json（未运行节点）"
  out="$("$BIN" node status --json --data-dir "$SMOKE_TMP/status")" \
    || fail "node status --json 退出码非 0"
  assert_valid_json "node status --json" "$out"
  echo "  $out"

  log "4/4 smoke: chat friends list --json（空态好友簿）"
  out="$("$BIN" chat friends list --json --data-dir "$SMOKE_TMP/empty")" \
    || fail "chat friends list --json 退出码非 0"
  assert_valid_json "chat friends list --json" "$out"
  echo "  $out"
}

build
smoke
log "artifact: $BIN ($(du -h "$BIN" | cut -f1))"
echo "R2-RELEASE-OK"
