#!/usr/bin/env bash
# clippy 门禁：全 workspace 全 target，警告一律当错误（-D warnings）
# 覆盖两域：根 workspace + apps/gui/src-tauri 独立 workspace。后者被根
# Cargo.toml exclude，根 clippy 扫不到（2026-09-05 group_contract 警告随 PR 漏网实锤）。
# src-tauri 段要真编译 tauri（系统库 webkit2gtk-4.1），跳过判定复用 gui-tauri.sh
# 的组织方式：非 macOS 且缺系统库时显式 SKIP 不假绿；GUI_TAURI_SKIP=1 提供
# 显式逃生口（SKIP 同样可观测）。
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/src-tauri-gate.sh 夹具驱动）
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "clippy: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT"
cargo clippy --workspace --all-targets -- -D warnings || {
  echo "clippy: FAIL 根 workspace 有警告（-D warnings）" >&2
  exit 1
}

if [ -d "$ROOT/apps/gui/src-tauri" ]; then
  if [ "${GUI_TAURI_SKIP:-0}" = "1" ]; then
    echo "clippy: SKIP src-tauri 段（GUI_TAURI_SKIP=1 显式跳过）"
  elif [ "$(uname -s)" != "Darwin" ] && ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    echo "clippy: SKIP src-tauri 段（非 macOS 且缺 webkit2gtk-4.1，tauri 无法编译；与 gui-tauri-check 同口径）"
  else
    (cd "$ROOT/apps/gui/src-tauri"
      cargo clippy --all-targets -- -D warnings || {
        echo "clippy: FAIL src-tauri 有警告（-D warnings）" >&2
        exit 1
      })
    echo "clippy: src-tauri 段 PASS"
  fi
fi
echo "clippy: PASS"
