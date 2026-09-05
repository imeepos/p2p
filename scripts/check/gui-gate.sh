#!/usr/bin/env bash
# GUI 合并门禁：Rust 桥接 + 前端构建一次全跑（GUI 协调者合并前机械验收用）
# 用法：bash scripts/check/gui-gate.sh [--skip-frontend|--skip-rust]
# 说明：src-tauri 是独立 cargo 项目（根 workspace 已 exclude），故在此单独跑。
set -u
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
SKIP_RUST=0
SKIP_FRONTEND=0
for arg in "$@"; do
  case "$arg" in
    --skip-rust) SKIP_RUST=1 ;;
    --skip-frontend) SKIP_FRONTEND=1 ;;
    *) echo "gui-gate: 未知参数 $arg" >&2; exit 2 ;;
  esac
done

fail=0

if [ "$SKIP_RUST" -eq 0 ]; then
  echo "== [rust] src-tauri clippy + test =="
  if [ ! -d "$ROOT/apps/gui/src-tauri" ]; then
    echo "gui-gate: apps/gui/src-tauri 不存在，跳过 rust 门禁（W1 未落地?）" >&2
  else
    (cd "$ROOT/apps/gui/src-tauri" || exit 1
      cargo clippy -- -D warnings || fail=1
      cargo test || fail=1)
  fi
fi

if [ "$SKIP_FRONTEND" -eq 0 ]; then
  echo "== [frontend] pnpm build =="
  if [ ! -f "$ROOT/apps/gui/package.json" ]; then
    echo "gui-gate: apps/gui/package.json 不存在，跳过前端门禁（W1 未落地?）" >&2
  else
    (cd "$ROOT" && pnpm -C apps/gui build) || fail=1
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "gui-gate: FAIL" >&2
  exit 1
fi
echo "gui-gate: PASS"
